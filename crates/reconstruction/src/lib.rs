use filesystem::CandidateFile;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use storage::StorageDevice;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ReconstructionError {
    #[error("Storage error: {0}")]
    Storage(#[from] storage::StorageError),

    #[error("IO error writing output file: {0}")]
    Io(#[from] std::io::Error),

    #[error("No data available for candidate")]
    NoDataAvailable,
}

pub type Result<T> = std::result::Result<T, ReconstructionError>;

/// Reconstruct file bytes into memory buffer.
pub fn reconstruct_bytes(
    candidate: &CandidateFile,
    device: &dyn StorageDevice,
    bytes_per_cluster: u32,
) -> Result<Vec<u8>> {
    if candidate.is_resident {
        if let Some(ref data) = candidate.resident_data {
            return Ok(data.clone());
        } else {
            return Err(ReconstructionError::NoDataAvailable);
        }
    }

    let mut file_buf = Vec::with_capacity(candidate.size_bytes as usize);
    let mut remaining = candidate.size_bytes;

    for run in &candidate.data_runs {
        if remaining == 0 {
            break;
        }

        let read_len_bytes = run.cluster_count * bytes_per_cluster as u64;
        let actual_len_bytes = read_len_bytes.min(remaining);

        if run.cluster_offset == u64::MAX {
            // Sparse run
            file_buf.resize(file_buf.len() + actual_len_bytes as usize, 0);
            remaining -= actual_len_bytes;
        } else {
            let run_offset_bytes = run.cluster_offset * bytes_per_cluster as u64;
            let chunk = device.read_exact_at(run_offset_bytes, read_len_bytes as usize)?;
            let chunk_slice = &chunk[0..actual_len_bytes as usize];
            
            remaining -= chunk_slice.len() as u64;
            file_buf.extend_from_slice(chunk_slice);
        }
    }

    Ok(file_buf)
}

/// Reconstruct file bytes directly to a target destination path on disk.
pub fn reconstruct_to_file<P: AsRef<Path>>(
    candidate: &CandidateFile,
    device: &dyn StorageDevice,
    bytes_per_cluster: u32,
    output_path: P,
) -> Result<u64> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let mut out_file = File::create(output_path)?;

    if candidate.is_resident {
        if let Some(ref data) = candidate.resident_data {
            out_file.write_all(data)?;
            return Ok(data.len() as u64);
        } else {
            return Err(ReconstructionError::NoDataAvailable);
        }
    }

    let mut total_written: u64 = 0;
    let mut remaining = candidate.size_bytes;

    for run in &candidate.data_runs {
        if remaining == 0 {
            break;
        }

        let read_len_bytes = run.cluster_count * bytes_per_cluster as u64;
        let actual_len_bytes = read_len_bytes.min(remaining);

        if run.cluster_offset == u64::MAX {
            let zeros = vec![0u8; actual_len_bytes as usize];
            out_file.write_all(&zeros)?;
            total_written += actual_len_bytes;
            remaining -= actual_len_bytes;
        } else {
            let run_offset_bytes = run.cluster_offset * bytes_per_cluster as u64;
            let chunk = device.read_exact_at(run_offset_bytes, read_len_bytes as usize)?;
            let chunk_slice = &chunk[0..actual_len_bytes as usize];
            
            out_file.write_all(chunk_slice)?;
            total_written += chunk_slice.len() as u64;
            remaining -= chunk_slice.len() as u64;
        }
    }

    Ok(total_written)
}
