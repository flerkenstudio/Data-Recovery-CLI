use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use recovery_core::{list_drives, RecoverySession};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "recover-cli")]
#[command(about = "Windows Data Recovery Suite CLI - Privacy-first NTFS deleted file recovery utility", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// List available system drives and volumes
    ListDrives,

    /// Scan a drive or disk image for deleted files
    Scan {
        /// Drive path (e.g. \\.\C:) or disk image path (e.g. image.img)
        target: String,

        /// Maximum MFT records to scan (0 = scan all MFT records on volume)
        #[arg(long, default_value_t = 0)]
        max_records: u64,

        /// Filter candidates by folder, path, extension, or filename pattern (case-insensitive)
        #[arg(short, long)]
        filter: Option<String>,

        /// Output results in JSON format
        #[arg(long)]
        json: bool,
    },

    /// Scan and recover deleted files from a drive or disk image
    Recover {
        /// Drive path or disk image path
        target: String,

        /// Output directory to store recovered files (MUST NOT be the source drive!)
        #[arg(short, long)]
        output: PathBuf,

        /// Maximum MFT records to scan (0 = scan all MFT records on volume)
        #[arg(long, default_value_t = 0)]
        max_records: u64,

        /// Filter candidates to recover by folder, path, extension, or filename pattern (case-insensitive)
        #[arg(short, long)]
        filter: Option<String>,

        /// Specific candidate IDs to recover (e.g. --candidate rec_7_1)
        #[arg(long)]
        candidate: Vec<String>,

        /// Output results in JSON format
        #[arg(long)]
        json: bool,
    },
}

fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    match cli.command {
        Commands::ListDrives => {
            let drives = list_drives().context("Failed to list system drives")?;
            println!("{:<15} {:<15} {:<15} {}", "DEVICE PATH", "DISPLAY NAME", "FILESYSTEM", "MOUNT POINT");
            println!("{}", "-".repeat(65));
            for d in drives {
                println!(
                    "{:<15} {:<15} {:<15} {}",
                    d.device_path,
                    d.display_name,
                    d.filesystem.unwrap_or_default(),
                    d.mount_point.unwrap_or_default()
                );
            }
        }
        Commands::Scan {
            target,
            max_records,
            filter,
            json,
        } => {
            println!("Opening device/image: {target}...");
            let mut session = RecoverySession::open(&target)?;

            let limit = if max_records == 0 { None } else { Some(max_records) };
            println!("Scanning MFT records (scanning volume)...");

            let candidates = session.scan(limit, |progress| {
                if !json && (progress.records_scanned % 10000 == 0 || progress.is_complete) {
                    println!(
                        "Progress: {} / {} records scanned ({} deleted candidates found)",
                        progress.records_scanned, progress.total_records, progress.candidates_found
                    );
                }
            })?;

            let filtered_candidates: Vec<_> = if let Some(ref filter_str) = filter {
                let f_lower = filter_str.to_lowercase();
                candidates
                    .iter()
                    .filter(|c| {
                        c.name.to_lowercase().contains(&f_lower)
                            || c.path.to_lowercase().contains(&f_lower)
                            || c.extension.to_lowercase() == f_lower
                    })
                    .cloned()
                    .collect()
            } else {
                candidates.to_vec()
            };

            if json {
                let json_output = serde_json::to_string_pretty(&filtered_candidates)?;
                println!("{json_output}");
            } else {
                println!("\nFound {} deleted candidate file(s):", filtered_candidates.len());
                println!(
                    "{:<18} {:<25} {:<35} {:<12} {:<12}",
                    "ID", "NAME", "PATH", "SIZE (BYTES)", "CONFIDENCE"
                );
                println!("{}", "-".repeat(105));
                for c in &filtered_candidates {
                    println!(
                        "{:<18} {:<25} {:<35} {:<12} {} (score: {})",
                        c.id,
                        c.name,
                        c.path,
                        c.size_bytes,
                        format!("{:?}", c.confidence.band),
                        c.confidence.score
                    );
                }
            }
        }
        Commands::Recover {
            target,
            output,
            max_records,
            filter,
            candidate,
            json,
        } => {
            println!("Opening device/image: {target}...");
            let mut session = RecoverySession::open(&target)?;

            let limit = if max_records == 0 { None } else { Some(max_records) };
            println!("Scanning MFT records...");
            let candidates = session.scan(limit, |progress| {
                if !json && (progress.records_scanned % 20000 == 0 || progress.is_complete) {
                    println!(
                        "Progress: {} / {} records scanned ({} deleted candidates found)",
                        progress.records_scanned, progress.total_records, progress.candidates_found
                    );
                }
            })?;

            let target_ids: Vec<String> = if !candidate.is_empty() {
                candidate
            } else if let Some(ref filter_str) = filter {
                let f_lower = filter_str.to_lowercase();
                candidates
                    .iter()
                    .filter(|c| {
                        c.name.to_lowercase().contains(&f_lower)
                            || c.path.to_lowercase().contains(&f_lower)
                            || c.extension.to_lowercase() == f_lower
                    })
                    .map(|c| c.id.clone())
                    .collect()
            } else {
                candidates.iter().map(|c| c.id.clone()).collect()
            };

            if target_ids.is_empty() {
                println!("No candidate files found matching your criteria to recover.");
                return Ok(());
            }

            println!("Recovering {} candidate file(s) to {:?}...", target_ids.len(), output);
            let report = session.recover_candidates(&target_ids, &output)?;

            if json {
                let json_output = serde_json::to_string_pretty(&report)?;
                println!("{json_output}");
            } else {
                println!("\n--- Recovery Summary ---");
                println!("Total requested : {}", report.total_requested);
                println!("Successful      : {}", report.total_successful);
                println!("Failed          : {}", report.total_failed);
                println!("Destination     : {}", report.destination_path);
                println!("\nDetails:");
                for outcome in report.outcomes {
                    if outcome.success {
                        println!(
                            "  [OK] {} ({} bytes) -> SHA-256: {}",
                            outcome.file_name,
                            outcome.bytes_recovered,
                            outcome.sha256_hash.unwrap_or_default()
                        );
                    } else {
                        println!(
                            "  [FAIL] {} -> Error: {}",
                            outcome.file_name,
                            outcome.error.unwrap_or_default()
                        );
                    }
                }
            }
        }
    }

    Ok(())
}
