use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{Read, Result};
use std::path::Path;

/// Compute SHA-256 hash string for byte buffer.
pub fn compute_sha256_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

/// Compute SHA-256 hash string for file on disk.
pub fn compute_sha256_file<P: AsRef<Path>>(path: P) -> Result<String> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 8192];
    loop {
        let count = file.read(&mut buf)?;
        if count == 0 {
            break;
        }
        hasher.update(&buf[..count]);
    }
    Ok(hex::encode(hasher.finalize()))
}

/// Verify header signatures for common file formats.
pub fn validate_header(bytes: &[u8], extension: &str) -> bool {
    if bytes.is_empty() {
        return false;
    }

    match extension.to_lowercase().as_str() {
        "png" => bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]),
        "jpg" | "jpeg" => bytes.starts_with(&[0xFF, 0xD8, 0xFF]),
        "pdf" => bytes.starts_with(b"%PDF"),
        "zip" | "docx" | "xlsx" | "pptx" => bytes.starts_with(&[0x50, 0x4B, 0x03, 0x04]),
        "gif" => bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a"),
        "bmp" => bytes.starts_with(b"BM"),
        _ => true, // Default pass for unhandled file types
    }
}
