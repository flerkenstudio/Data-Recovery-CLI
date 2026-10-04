use filesystem::{
    CandidateFile, RecoveryProgress, RecoveryReport, ScanProgress, SingleFileRecoveryOutcome,
};
use ntfs_parser::BootSector;
use reconstruction::reconstruct_to_file;
use scanner::{quick_scan_ntfs, ScanOptions};
use std::path::Path;
use storage::{
    list_drives as storage_list_drives, open_storage_device, DriveDetails, StorageDevice,
};
use thiserror::Error;
use validation::compute_sha256_file;

#[derive(Error, Debug)]
pub enum RecoveryCoreError {
    #[error("Storage error: {0}")]
    Storage(#[from] storage::StorageError),

    #[error("Scanner error: {0}")]
    Scanner(#[from] scanner::ScannerError),

    #[error("Reconstruction error: {0}")]
    Reconstruction(#[from] reconstruction::ReconstructionError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Session error: {0}")]
    SessionError(String),
}

pub type Result<T> = std::result::Result<T, RecoveryCoreError>;

/// Query list of available system drives.
pub fn list_drives() -> Result<Vec<DriveDetails>> {
    let drives = storage_list_drives()?;
    Ok(drives)
}

/// Orchestrates disk scan and file recovery.
pub struct RecoverySession {
    pub device_path: String,
    pub storage_device: Box<dyn StorageDevice>,
    pub boot_sector: Option<BootSector>,
    pub candidates: Vec<CandidateFile>,
}

impl RecoverySession {
    pub fn open<P: AsRef<Path>>(device_path: P) -> Result<Self> {
        let path_str = device_path.as_ref().to_string_lossy().to_string();
        let storage_device = open_storage_device(&path_str)?;

        Ok(Self {
            device_path: path_str,
            storage_device,
            boot_sector: None,
            candidates: Vec::new(),
        })
    }

    pub fn scan(
        &mut self,
        max_records: Option<u64>,
        mut progress_cb: impl FnMut(ScanProgress),
    ) -> Result<&[CandidateFile]> {
        let options = ScanOptions {
            max_records,
            include_directories: false,
        };

        let (boot_sector, candidates) =
            quick_scan_ntfs(&*self.storage_device, options, &mut progress_cb)?;

        self.boot_sector = Some(boot_sector);
        self.candidates = candidates;

        Ok(&self.candidates)
    }

    pub fn recover_candidates<P: AsRef<Path>>(
        &self,
        candidate_ids: &[String],
        destination_dir: P,
        mut progress_cb: impl FnMut(RecoveryProgress),
    ) -> Result<RecoveryReport> {
        let dest_dir = destination_dir.as_ref();
        std::fs::create_dir_all(dest_dir)?;

        let boot_sector = self.boot_sector.as_ref().ok_or_else(|| {
            RecoveryCoreError::SessionError("Scan must be performed before recovery".to_string())
        })?;

        let bytes_per_cluster = boot_sector.bytes_per_cluster;
        let mut outcomes = Vec::new();
        let mut successful = 0;
        let mut failed = 0;

        for id in candidate_ids {
            let candidate = match self.candidates.iter().find(|c| &c.id == id) {
                Some(c) => c,
                None => {
                    failed += 1;
                    outcomes.push(SingleFileRecoveryOutcome {
                        candidate_id: id.clone(),
                        file_name: "unknown".to_string(),
                        success: false,
                        bytes_recovered: 0,
                        recovered_path: None,
                        sha256_hash: None,
                        error: Some("Candidate ID not found in scan results".to_string()),
                    });
                    continue;
                }
            };

            let target_file_path = dest_dir.join(&candidate.name);

            match reconstruct_to_file(
                candidate,
                &*self.storage_device,
                bytes_per_cluster,
                &target_file_path,
            ) {
                Ok(bytes_written) => {
                    let hash = compute_sha256_file(&target_file_path).ok();
                    successful += 1;
                    outcomes.push(SingleFileRecoveryOutcome {
                        candidate_id: id.clone(),
                        file_name: candidate.name.clone(),
                        success: true,
                        bytes_recovered: bytes_written,
                        recovered_path: Some(target_file_path.to_string_lossy().to_string()),
                        sha256_hash: hash,
                        error: None,
                    });
                }
                Err(err) => {
                    failed += 1;
                    outcomes.push(SingleFileRecoveryOutcome {
                        candidate_id: id.clone(),
                        file_name: candidate.name.clone(),
                        success: false,
                        bytes_recovered: 0,
                        recovered_path: None,
                        sha256_hash: None,
                        error: Some(err.to_string()),
                    });
                }
            }

            progress_cb(RecoveryProgress {
                files_processed: successful + failed,
                total_files: candidate_ids.len(),
                successful,
                failed,
            });
        }

        Ok(RecoveryReport {
            total_requested: candidate_ids.len(),
            total_successful: successful,
            total_failed: failed,
            destination_path: dest_dir.to_string_lossy().to_string(),
            outcomes,
        })
    }
}
