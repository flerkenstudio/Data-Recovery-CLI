use confidence::calculate_confidence;
use filesystem::{CandidateFile, CandidateKind, DataRun, FileTimestamps, ScanProgress};
use ntfs_parser::{BootSector, MftRecord, ParsedAttribute, ParsedDataRun};
use std::collections::HashMap;
use storage::StorageDevice;
use thiserror::Error;
use tracing::info;

#[derive(Error, Debug)]
pub enum ScannerError {
    #[error("Storage error: {0}")]
    Storage(#[from] storage::StorageError),

    #[error("NTFS parse error: {0}")]
    Ntfs(#[from] ntfs_parser::NtfsError),
}

pub type Result<T> = std::result::Result<T, ScannerError>;

/// Main Quick Scan options.
#[derive(Debug, Clone, Default)]
pub struct ScanOptions {
    pub max_records: Option<u64>,
    pub include_directories: bool,
}

/// Helper struct for reading MFT records across fragmented MFT data runs.
pub struct MftReader<'a> {
    device: &'a dyn StorageDevice,
    bytes_per_cluster: u32,
    record_size: usize,
    runs: Vec<ParsedDataRun>,
    total_size: u64,
}

impl<'a> MftReader<'a> {
    pub fn new(device: &'a dyn StorageDevice, boot_sector: &BootSector) -> Result<Self> {
        let first_record_offset = boot_sector.mft_lcn * boot_sector.bytes_per_cluster as u64;
        let record_size = boot_sector.mft_record_size as usize;

        let mut runs = Vec::new();
        let mut total_size = 0;

        if let Ok(record_0_bytes) = device.read_exact_at(first_record_offset, record_size) {
            if let Ok(rec0) = MftRecord::parse(record_0_bytes, 0) {
                for attr in rec0.attributes {
                    if let ParsedAttribute::Data(data_attr) = attr {
                        if data_attr.name.is_none() {
                            total_size = data_attr.size;
                            runs = data_attr.data_runs;
                            break;
                        }
                    }
                }
            }
        }

        if runs.is_empty() {
            runs.push(ParsedDataRun {
                lcn: boot_sector.mft_lcn,
                cluster_count: (device.size() / boot_sector.bytes_per_cluster as u64),
            });
            total_size = device.size();
        }

        Ok(Self {
            device,
            bytes_per_cluster: boot_sector.bytes_per_cluster,
            record_size,
            runs,
            total_size,
        })
    }

    pub fn total_records(&self) -> u64 {
        if self.total_size > 0 {
            self.total_size / self.record_size as u64
        } else {
            65536
        }
    }

    pub fn read_record(&self, record_index: u64) -> Result<Vec<u8>> {
        let record_offset_in_mft = record_index * self.record_size as u64;
        let mut current_mft_offset: u64 = 0;

        for run in &self.runs {
            let run_len_bytes = run.cluster_count * self.bytes_per_cluster as u64;
            if record_offset_in_mft >= current_mft_offset
                && record_offset_in_mft + self.record_size as u64
                    <= current_mft_offset + run_len_bytes
            {
                let offset_in_run = record_offset_in_mft - current_mft_offset;
                let disk_offset = (run.lcn * self.bytes_per_cluster as u64) + offset_in_run;
                return Ok(self.device.read_exact_at(disk_offset, self.record_size)?);
            }
            current_mft_offset += run_len_bytes;
        }

        Err(ScannerError::Storage(storage::StorageError::OutOfBounds {
            offset: record_offset_in_mft,
            len: self.record_size,
            size: self.total_size,
        }))
    }

    /// Optimized chunk reader: reads a block of N records in a single disk read call.
    pub fn read_records_chunk(&self, start_record: u64, num_records: usize) -> Result<Vec<u8>> {
        let chunk_bytes = num_records * self.record_size;
        let start_offset_in_mft = start_record * self.record_size as u64;
        let mut current_mft_offset: u64 = 0;

        for run in &self.runs {
            let run_len_bytes = run.cluster_count * self.bytes_per_cluster as u64;
            if start_offset_in_mft >= current_mft_offset
                && start_offset_in_mft + chunk_bytes as u64 <= current_mft_offset + run_len_bytes
            {
                let offset_in_run = start_offset_in_mft - current_mft_offset;
                let disk_offset = (run.lcn * self.bytes_per_cluster as u64) + offset_in_run;
                return Ok(self.device.read_exact_at(disk_offset, chunk_bytes)?);
            }
            current_mft_offset += run_len_bytes;
        }

        // Fallback if chunk spans across data run boundaries
        let mut buf = Vec::with_capacity(chunk_bytes);
        for i in 0..num_records {
            if let Ok(rec_bytes) = self.read_record(start_record + i as u64) {
                buf.extend_from_slice(&rec_bytes);
            } else {
                buf.resize(buf.len() + self.record_size, 0);
            }
        }
        Ok(buf)
    }
}

/// Helper to reconstruct directory hierarchy path from parent references.
fn build_path(
    parent_ref: Option<u64>,
    file_name: &str,
    dir_map: &HashMap<u64, (String, u64)>,
) -> String {
    let mut parts = Vec::new();
    parts.push(file_name.to_string());

    let mut current_parent = parent_ref;
    let mut visited = std::collections::HashSet::new();

    while let Some(parent_idx) = current_parent {
        if parent_idx == 5 || visited.contains(&parent_idx) {
            break;
        }
        visited.insert(parent_idx);

        if let Some((dir_name, p_ref)) = dir_map.get(&parent_idx) {
            if dir_name != "." && dir_name != "\\" {
                parts.push(dir_name.clone());
            }
            current_parent = Some(*p_ref);
        } else {
            break;
        }
    }

    parts.reverse();
    parts.join("/")
}

struct RawCandidate {
    rec_idx: u64,
    sequence_number: u16,
    name: String,
    parent_ref: Option<u64>,
    data_size: u64,
    is_resident: bool,
    resident_bytes: Option<Vec<u8>>,
    data_runs: Vec<DataRun>,
    is_directory: bool,
    timestamps: FileTimestamps,
}

/// Scan NTFS volume or image file for deleted file candidates in a single high-speed pass.
pub fn quick_scan_ntfs(
    device: &dyn StorageDevice,
    options: ScanOptions,
    mut progress_cb: impl FnMut(ScanProgress),
) -> Result<(BootSector, Vec<CandidateFile>)> {
    let boot_buf = device.read_exact_at(0, 512)?;
    let boot_sector = BootSector::parse(&boot_buf)?;

    let mft_reader = MftReader::new(device, &boot_sector)?;
    let total_mft_records = mft_reader.total_records();

    let max_recs = match options.max_records {
        Some(limit) if limit > 0 => limit.min(total_mft_records),
        _ => total_mft_records,
    };

    info!(
        "Starting high-speed single-pass MFT scan over {} records...",
        max_recs
    );

    let mut dir_map: HashMap<u64, (String, u64)> = HashMap::new();
    let mut raw_candidates: Vec<RawCandidate> = Vec::new();

    let chunk_size_records = 256;
    let record_size = boot_sector.mft_record_size as usize;

    let mut curr_rec: u64 = 0;
    while curr_rec < max_recs {
        let count = ((max_recs - curr_rec) as usize).min(chunk_size_records);
        let chunk_bytes = match mft_reader.read_records_chunk(curr_rec, count) {
            Ok(bytes) if bytes.len() == count * record_size => bytes,
            _ => {
                curr_rec += count as u64;
                continue;
            }
        };

        for i in 0..count {
            let rec_idx = curr_rec + i as u64;
            let offset = i * record_size;
            let rec_bytes = &chunk_bytes[offset..offset + record_size];

            if &rec_bytes[0..4] != b"FILE" {
                continue;
            }

            let record = match MftRecord::parse(rec_bytes.to_vec(), rec_idx) {
                Ok(r) => r,
                Err(_) => continue,
            };

            let mut is_directory = record.is_directory;
            let mut best_name = None;

            for attr in &record.attributes {
                match attr {
                    ParsedAttribute::Other { attr_type } => {
                        if *attr_type == 0x90 || *attr_type == 0xA0 {
                            is_directory = true;
                        }
                    }
                    ParsedAttribute::FileName(fn_attr)
                        if (best_name.is_none() || fn_attr.namespace != 2) =>
                    {
                        best_name = Some((fn_attr.name.clone(), fn_attr.parent_mft_ref));
                    }
                    _ => {}
                }
            }

            if is_directory {
                if let Some((name, parent_ref)) = best_name {
                    dir_map.insert(rec_idx, (name, parent_ref));
                }
            }

            if !record.in_use {
                if is_directory && !options.include_directories {
                    continue;
                }

                let mut file_name = None;
                let mut parent_ref = None;
                let mut fn_timestamps = None;
                let mut si_timestamps = None;
                let mut data_size = 0u64;
                let mut is_resident = false;
                let mut resident_bytes = None;
                let mut data_runs = Vec::new();

                for attr in &record.attributes {
                    match attr {
                        ParsedAttribute::FileName(fn_attr) => {
                            if file_name.is_none() || fn_attr.namespace != 2 {
                                file_name = Some(fn_attr.name.clone());
                                parent_ref = Some(fn_attr.parent_mft_ref);
                                fn_timestamps = Some(FileTimestamps {
                                    created: fn_attr.creation_time,
                                    modified: fn_attr.modification_time,
                                    mft_modified: fn_attr.mft_modification_time,
                                    accessed: fn_attr.access_time,
                                });
                            }
                        }
                        ParsedAttribute::StandardInformation(si_attr) => {
                            si_timestamps = Some(FileTimestamps {
                                created: si_attr.creation_time,
                                modified: si_attr.modification_time,
                                mft_modified: si_attr.mft_modification_time,
                                accessed: si_attr.access_time,
                            });
                        }
                        ParsedAttribute::Data(data_attr) if data_attr.name.is_none() => {
                            data_size = data_attr.size;
                            is_resident = !data_attr.is_non_resident;
                            resident_bytes = data_attr.resident_data.clone();
                            data_runs = data_attr
                                .data_runs
                                .iter()
                                .map(|r| DataRun {
                                    cluster_offset: r.lcn,
                                    cluster_count: r.cluster_count,
                                })
                                .collect();
                        }
                        _ => {}
                    }
                }

                if let Some(name) = file_name {
                    if name.starts_with('$') && rec_idx < 16 {
                        continue;
                    }

                    let timestamps = fn_timestamps.or(si_timestamps).unwrap_or(FileTimestamps {
                        created: None,
                        modified: None,
                        mft_modified: None,
                        accessed: None,
                    });

                    raw_candidates.push(RawCandidate {
                        rec_idx,
                        sequence_number: record.sequence_number,
                        name,
                        parent_ref,
                        data_size,
                        is_resident,
                        resident_bytes,
                        data_runs,
                        is_directory,
                        timestamps,
                    });
                }
            }
        }

        curr_rec += count as u64;

        // Frequent progress callback every 2,500 records for smooth live UI animation
        if curr_rec.is_multiple_of(2500) || curr_rec == max_recs {
            progress_cb(ScanProgress {
                records_scanned: curr_rec,
                candidates_found: raw_candidates.len() as u64,
                total_records: max_recs,
                is_complete: curr_rec == max_recs,
                current_status: format!("Scanned MFT record {}/{}", curr_rec, max_recs),
            });
        }
    }

    let candidates = raw_candidates
        .into_iter()
        .map(|raw| {
            let path = build_path(raw.parent_ref, &raw.name, &dir_map);

            let ext = std::path::Path::new(&raw.name)
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();

            let confidence = calculate_confidence(
                false,
                raw.is_resident,
                !raw.name.is_empty(),
                raw.data_size,
                !raw.data_runs.is_empty(),
            );

            let id = format!("rec_{}_{}", raw.rec_idx, raw.sequence_number);

            CandidateFile {
                id,
                mft_record_index: raw.rec_idx,
                sequence_number: raw.sequence_number,
                name: raw.name,
                path,
                extension: ext,
                parent_mft_reference: raw.parent_ref,
                size_bytes: raw.data_size,
                allocated_size_bytes: raw.data_size,
                is_directory: raw.is_directory,
                timestamps: raw.timestamps,
                kind: CandidateKind::Metadata,
                confidence,
                is_resident: raw.is_resident,
                resident_data: raw.resident_bytes,
                data_runs: raw.data_runs,
            }
        })
        .collect();

    Ok((boot_sector, candidates))
}
