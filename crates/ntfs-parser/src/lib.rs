use byteorder::{LittleEndian, ReadBytesExt};
use chrono::{DateTime, TimeZone, Utc};
use std::io::Cursor;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum NtfsError {
    #[error("Storage error: {0}")]
    Storage(#[from] storage::StorageError),

    #[error("Invalid NTFS boot sector magic (expected 'NTFS    ')")]
    InvalidBootSector,

    #[error("Invalid MFT record magic (expected 'FILE' or 'BAAD')")]
    InvalidMftRecord,

    #[error("USA (Update Sequence Array) fixup mismatch")]
    FixupFailed,

    #[error("Attribute parse error: {0}")]
    AttributeError(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, NtfsError>;

/// Parsed NTFS Boot Sector information.
#[derive(Debug, Clone)]
pub struct BootSector {
    pub bytes_per_sector: u16,
    pub sectors_per_cluster: u8,
    pub bytes_per_cluster: u32,
    pub mft_lcn: u64,
    pub mft_record_size: u32,
    pub total_sectors: u64,
}

impl BootSector {
    pub fn parse(buf: &[u8]) -> Result<Self> {
        if buf.len() < 512 {
            return Err(NtfsError::InvalidBootSector);
        }

        let oem_id = &buf[3..11];
        if oem_id != b"NTFS    " {
            return Err(NtfsError::InvalidBootSector);
        }

        let mut cursor = Cursor::new(buf);
        cursor.set_position(11);
        let bytes_per_sector = cursor.read_u16::<LittleEndian>()?;
        let sectors_per_cluster = cursor.read_u8()?;

        let bytes_per_cluster = bytes_per_sector as u32 * sectors_per_cluster as u32;

        cursor.set_position(40);
        let total_sectors = cursor.read_u64::<LittleEndian>()?;
        let mft_lcn = cursor.read_u64::<LittleEndian>()?;

        cursor.set_position(64);
        let clusters_per_mft_record = cursor.read_i8()?;
        let mft_record_size = if clusters_per_mft_record > 0 {
            clusters_per_mft_record as u32 * bytes_per_cluster
        } else {
            1u32 << (-clusters_per_mft_record as u32)
        };

        Ok(Self {
            bytes_per_sector,
            sectors_per_cluster,
            bytes_per_cluster,
            mft_lcn,
            mft_record_size,
            total_sectors,
        })
    }
}

/// Win32 FILETIME timestamp helper (100-ns intervals since Jan 1, 1601 UTC).
pub fn parse_filetime(filetime: u64) -> Option<DateTime<Utc>> {
    if filetime == 0 {
        return None;
    }
    if filetime < 116_444_736_000_000_000 {
        return None;
    }
    let nanos_since_epoch = (filetime - 116_444_736_000_000_000) * 100;
    let secs = (nanos_since_epoch / 1_000_000_000) as i64;
    let nsecs = (nanos_since_epoch % 1_000_000_000) as u32;

    Utc.timestamp_opt(secs, nsecs).single()
}

/// Decoded Data Run range (cluster offset and length in clusters).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedDataRun {
    pub lcn: u64,
    pub cluster_count: u64,
}

/// Decode raw NTFS runlist bytes into LCN runs.
pub fn decode_runlist(runlist_bytes: &[u8]) -> Result<Vec<ParsedDataRun>> {
    let mut runs = Vec::new();
    let mut cursor = 0;
    let mut current_lcn: i64 = 0;

    while cursor < runlist_bytes.len() {
        let header = runlist_bytes[cursor];
        if header == 0 {
            break;
        }
        cursor += 1;

        let len_size = (header & 0x0F) as usize;
        let offset_size = ((header >> 4) & 0x0F) as usize;

        if cursor + len_size + offset_size > runlist_bytes.len() {
            break;
        }

        let mut count: u64 = 0;
        for i in 0..len_size {
            count |= (runlist_bytes[cursor + i] as u64) << (i * 8);
        }
        cursor += len_size;

        let mut lcn_delta: i64 = 0;
        let mut is_sparse = false;
        
        if offset_size > 0 {
            for i in 0..offset_size {
                lcn_delta |= (runlist_bytes[cursor + i] as i64) << (i * 8);
            }
            let msb = (runlist_bytes[cursor + offset_size - 1] & 0x80) != 0;
            if msb {
                for i in offset_size..8 {
                    lcn_delta |= 0xFFi64 << (i * 8);
                }
            }
            cursor += offset_size;
            current_lcn += lcn_delta;
        } else {
            is_sparse = true;
        }

        runs.push(ParsedDataRun {
            lcn: if is_sparse { u64::MAX } else { current_lcn as u64 },
            cluster_count: count,
        });
    }

    Ok(runs)
}

/// Update Sequence Array (USA) fixup.
pub fn apply_usa_fixup(buf: &mut [u8], usa_offset: u16, usa_count: u16) -> Result<()> {
    if usa_count < 1 {
        return Ok(());
    }

    let usa_offset = usa_offset as usize;
    if usa_offset + (usa_count as usize * 2) > buf.len() {
        return Err(NtfsError::FixupFailed);
    }

    let usn = u16::from_le_bytes([buf[usa_offset], buf[usa_offset + 1]]);

    for i in 1..usa_count as usize {
        let sector_end = i * 512 - 2;
        if sector_end + 2 > buf.len() {
            break;
        }

        let sector_usn = u16::from_le_bytes([buf[sector_end], buf[sector_end + 1]]);
        if sector_usn != usn {
            return Err(NtfsError::FixupFailed);
        }

        let fixup_val = [
            buf[usa_offset + i * 2],
            buf[usa_offset + i * 2 + 1],
        ];

        buf[sector_end] = fixup_val[0];
        buf[sector_end + 1] = fixup_val[1];
    }

    Ok(())
}

#[derive(Debug, Clone)]
pub struct StandardInformationAttr {
    pub creation_time: Option<DateTime<Utc>>,
    pub modification_time: Option<DateTime<Utc>>,
    pub mft_modification_time: Option<DateTime<Utc>>,
    pub access_time: Option<DateTime<Utc>>,
    pub dos_permissions: u32,
}

#[derive(Debug, Clone)]
pub struct FileNameAttr {
    pub parent_mft_ref: u64,
    pub creation_time: Option<DateTime<Utc>>,
    pub modification_time: Option<DateTime<Utc>>,
    pub mft_modification_time: Option<DateTime<Utc>>,
    pub access_time: Option<DateTime<Utc>>,
    pub allocated_size: u64,
    pub real_size: u64,
    pub flags: u32,
    pub name: String,
    pub namespace: u8,
}

#[derive(Debug, Clone)]
pub struct DataAttr {
    pub is_non_resident: bool,
    pub name: Option<String>,
    pub size: u64,
    pub resident_data: Option<Vec<u8>>,
    pub data_runs: Vec<ParsedDataRun>,
}

#[derive(Debug, Clone)]
pub enum ParsedAttribute {
    StandardInformation(StandardInformationAttr),
    FileName(FileNameAttr),
    Data(DataAttr),
    Other { attr_type: u32 },
}

/// Parsed MFT Record.
#[derive(Debug, Clone)]
pub struct MftRecord {
    pub index: u64,
    pub sequence_number: u16,
    pub in_use: bool,
    pub is_directory: bool,
    pub hard_link_count: u16,
    pub attributes: Vec<ParsedAttribute>,
}

impl MftRecord {
    pub fn parse(mut record_bytes: Vec<u8>, record_index: u64) -> Result<Self> {
        if record_bytes.len() < 42 {
            return Err(NtfsError::InvalidMftRecord);
        }

        let magic = &record_bytes[0..4];
        if magic != b"FILE" {
            return Err(NtfsError::InvalidMftRecord);
        }

        let usa_offset = u16::from_le_bytes([record_bytes[4], record_bytes[5]]);
        let usa_count = u16::from_le_bytes([record_bytes[6], record_bytes[7]]);

        let _ = apply_usa_fixup(&mut record_bytes, usa_offset, usa_count);

        let sequence_number = u16::from_le_bytes([record_bytes[16], record_bytes[17]]);
        let hard_link_count = u16::from_le_bytes([record_bytes[18], record_bytes[19]]);
        let first_attr_offset = u16::from_le_bytes([record_bytes[20], record_bytes[21]]) as usize;
        let flags = u16::from_le_bytes([record_bytes[22], record_bytes[23]]);

        let in_use = (flags & 0x0001) != 0;
        let is_directory = (flags & 0x0002) != 0;

        let mut attributes = Vec::new();
        let mut curr_offset = first_attr_offset;

        while curr_offset + 8 <= record_bytes.len() {
            let attr_type = u32::from_le_bytes([
                record_bytes[curr_offset],
                record_bytes[curr_offset + 1],
                record_bytes[curr_offset + 2],
                record_bytes[curr_offset + 3],
            ]);

            if attr_type == 0xFFFFFFFF {
                break;
            }

            let attr_len = u32::from_le_bytes([
                record_bytes[curr_offset + 4],
                record_bytes[curr_offset + 5],
                record_bytes[curr_offset + 6],
                record_bytes[curr_offset + 7],
            ]) as usize;

            if attr_len < 8 || curr_offset + attr_len > record_bytes.len() {
                break;
            }

            let attr_bytes = &record_bytes[curr_offset..curr_offset + attr_len];
            let non_resident = attr_bytes[8] != 0;

            match attr_type {
                0x10 => {
                    if !non_resident && attr_len >= 24 + 48 {
                        let content_offset = u16::from_le_bytes([attr_bytes[20], attr_bytes[21]]) as usize;
                        if content_offset + 48 <= attr_len {
                            let c_bytes = &attr_bytes[content_offset..];
                            let create_ft = u64::from_le_bytes(c_bytes[0..8].try_into().unwrap());
                            let mod_ft = u64::from_le_bytes(c_bytes[8..16].try_into().unwrap());
                            let mft_mod_ft = u64::from_le_bytes(c_bytes[16..24].try_into().unwrap());
                            let acc_ft = u64::from_le_bytes(c_bytes[24..32].try_into().unwrap());
                            let dos_perm = u32::from_le_bytes(c_bytes[32..36].try_into().unwrap());

                            attributes.push(ParsedAttribute::StandardInformation(
                                StandardInformationAttr {
                                    creation_time: parse_filetime(create_ft),
                                    modification_time: parse_filetime(mod_ft),
                                    mft_modification_time: parse_filetime(mft_mod_ft),
                                    access_time: parse_filetime(acc_ft),
                                    dos_permissions: dos_perm,
                                },
                            ));
                        }
                    }
                }
                0x30 => {
                    if !non_resident && attr_bytes.len() >= 24 {
                        let content_offset = u16::from_le_bytes([attr_bytes[20], attr_bytes[21]]) as usize;
                        if content_offset + 66 <= attr_bytes.len() {
                            let c_bytes = &attr_bytes[content_offset..];
                            let parent_ref = u64::from_le_bytes(c_bytes[0..8].try_into().unwrap()) & 0x0000FFFFFFFFFFFF;
                            let create_ft = u64::from_le_bytes(c_bytes[8..16].try_into().unwrap());
                            let mod_ft = u64::from_le_bytes(c_bytes[16..24].try_into().unwrap());
                            let mft_mod_ft = u64::from_le_bytes(c_bytes[24..32].try_into().unwrap());
                            let acc_ft = u64::from_le_bytes(c_bytes[32..40].try_into().unwrap());
                            let alloc_sz = u64::from_le_bytes(c_bytes[40..48].try_into().unwrap());
                            let real_sz = u64::from_le_bytes(c_bytes[48..56].try_into().unwrap());
                            let flags = u32::from_le_bytes(c_bytes[56..60].try_into().unwrap());
                            let name_len = c_bytes[64] as usize;
                            let namespace = c_bytes[65];

                            if content_offset + 66 + name_len * 2 <= attr_bytes.len() {
                                let name_u16: Vec<u16> = c_bytes[66..66 + name_len * 2]
                                    .chunks_exact(2)
                                    .map(|c| u16::from_le_bytes([c[0], c[1]]))
                                    .collect();
                                let name = String::from_utf16_lossy(&name_u16);

                                attributes.push(ParsedAttribute::FileName(FileNameAttr {
                                    parent_mft_ref: parent_ref,
                                    creation_time: parse_filetime(create_ft),
                                    modification_time: parse_filetime(mod_ft),
                                    mft_modification_time: parse_filetime(mft_mod_ft),
                                    access_time: parse_filetime(acc_ft),
                                    allocated_size: alloc_sz,
                                    real_size: real_sz,
                                    flags,
                                    name,
                                    namespace,
                                }));
                            }
                        }
                    }
                }
                0x80 => {
                    let name_len = attr_bytes[9] as usize;
                    let name_off = u16::from_le_bytes([attr_bytes[10], attr_bytes[11]]) as usize;
                    let name = if name_len > 0 && name_off + name_len * 2 <= attr_bytes.len() {
                        let u16s: Vec<u16> = attr_bytes[name_off..name_off + name_len * 2]
                            .chunks_exact(2)
                            .map(|c| u16::from_le_bytes([c[0], c[1]]))
                            .collect();
                        Some(String::from_utf16_lossy(&u16s))
                    } else {
                        None
                    };

                    if non_resident {
                        let runlist_off = u16::from_le_bytes([attr_bytes[32], attr_bytes[33]]) as usize;
                        let real_size = u64::from_le_bytes(attr_bytes[48..56].try_into().unwrap_or([0; 8]));
                        let runs = if runlist_off < attr_bytes.len() {
                            decode_runlist(&attr_bytes[runlist_off..]).unwrap_or_default()
                        } else {
                            Vec::new()
                        };

                        attributes.push(ParsedAttribute::Data(DataAttr {
                            is_non_resident: true,
                            name,
                            size: real_size,
                            resident_data: None,
                            data_runs: runs,
                        }));
                    } else {
                        let content_off = u16::from_le_bytes([attr_bytes[20], attr_bytes[21]]) as usize;
                        let content_len = u32::from_le_bytes([attr_bytes[16], attr_bytes[17], attr_bytes[18], attr_bytes[19]]) as usize;

                        let payload = if content_off + content_len <= attr_bytes.len() {
                            Some(attr_bytes[content_off..content_off + content_len].to_vec())
                        } else {
                            None
                        };

                        attributes.push(ParsedAttribute::Data(DataAttr {
                            is_non_resident: false,
                            name,
                            size: content_len as u64,
                            resident_data: payload,
                            data_runs: Vec::new(),
                        }));
                    }
                }
                _ => {
                    attributes.push(ParsedAttribute::Other { attr_type });
                }
            }

            curr_offset += attr_len;
        }

        Ok(Self {
            index: record_index,
            sequence_number,
            in_use,
            is_directory,
            hard_link_count,
            attributes,
        })
    }
}
