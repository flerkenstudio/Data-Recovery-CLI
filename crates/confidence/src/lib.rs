use filesystem::{ConfidenceBand, ConfidenceScore};

/// Score the recovery confidence for a candidate file based on MFT & runlist properties.
pub fn calculate_confidence(
    is_deleted: bool,
    is_resident: bool,
    has_valid_name: bool,
    data_size: u64,
    has_data_runs: bool,
) -> ConfidenceScore {
    let mut score: u32 = 100;
    let mut reasons = Vec::new();

    if !is_deleted {
        reasons.push("File is currently active (not marked deleted)".to_string());
    } else {
        reasons.push("MFT record is marked deleted (valid deleted candidate)".to_string());
    }

    if !has_valid_name {
        score = score.saturating_sub(40);
        reasons.push("Filename missing or damaged".to_string());
    }

    if is_resident {
        reasons.push("Data is resident inside MFT record (100% recoverable if MFT intact)".to_string());
    } else {
        if !has_data_runs && data_size > 0 {
            score = score.saturating_sub(60);
            reasons.push("Non-resident file missing cluster runlist".to_string());
        } else {
            reasons.push("Non-resident data runlist present".to_string());
        }
    }

    if data_size == 0 {
        score = score.saturating_sub(20);
        reasons.push("File size is 0 bytes".to_string());
    }

    let final_score = (score.min(100)) as u8;
    let band = match final_score {
        80..=100 => ConfidenceBand::High,
        50..=79 => ConfidenceBand::Medium,
        20..=49 => ConfidenceBand::Low,
        _ => ConfidenceBand::Unlikely,
    };

    ConfidenceScore {
        score: final_score,
        band,
        reasons,
    }
}
