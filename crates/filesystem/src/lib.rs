use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileTimestamps {
    pub created: Option<DateTime<Utc>>,
    pub modified: Option<DateTime<Utc>>,
    pub mft_modified: Option<DateTime<Utc>>,
    pub accessed: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum CandidateKind {
    Metadata,
    Carved,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ConfidenceBand {
    High,
    Medium,
    Low,
    Unlikely,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfidenceScore {
    pub score: u8, // 0 to 100
    pub band: ConfidenceBand,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataRun {
    pub cluster_offset: u64,
    pub cluster_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandidateFile {
    pub id: String,
    pub mft_record_index: u64,
    pub sequence_number: u16,
    pub name: String,
    pub path: String,
    pub extension: String,
    pub parent_mft_reference: Option<u64>,
    pub size_bytes: u64,
    pub allocated_size_bytes: u64,
    pub is_directory: bool,
    pub timestamps: FileTimestamps,
    pub kind: CandidateKind,
    pub confidence: ConfidenceScore,
    pub is_resident: bool,
    pub resident_data: Option<Vec<u8>>,
    pub data_runs: Vec<DataRun>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanProgress {
    pub records_scanned: u64,
    pub candidates_found: u64,
    pub total_records: u64,
    pub is_complete: bool,
    pub current_status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryProgress {
    pub files_processed: usize,
    pub total_files: usize,
    pub successful: usize,
    pub failed: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryRequest {
    pub candidate_ids: Vec<String>,
    pub destination_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SingleFileRecoveryOutcome {
    pub candidate_id: String,
    pub file_name: String,
    pub success: bool,
    pub bytes_recovered: u64,
    pub recovered_path: Option<String>,
    pub sha256_hash: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryReport {
    pub total_requested: usize,
    pub total_successful: usize,
    pub total_failed: usize,
    pub destination_path: String,
    pub outcomes: Vec<SingleFileRecoveryOutcome>,
}
