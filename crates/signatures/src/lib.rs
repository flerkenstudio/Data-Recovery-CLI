use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileSignature {
    pub name: &'static str,
    pub extension: &'static str,
    pub header_bytes: &'static [u8],
    pub footer_bytes: Option<&'static [u8]>,
    pub max_size_bytes: u64,
}

pub static KNOWN_SIGNATURES: &[FileSignature] = &[
    FileSignature {
        name: "PNG Image",
        extension: "png",
        header_bytes: &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A],
        footer_bytes: Some(&[0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82]),
        max_size_bytes: 50 * 1024 * 1024,
    },
    FileSignature {
        name: "JPEG Image",
        extension: "jpg",
        header_bytes: &[0xFF, 0xD8, 0xFF],
        footer_bytes: Some(&[0xFF, 0xD9]),
        max_size_bytes: 50 * 1024 * 1024,
    },
    FileSignature {
        name: "PDF Document",
        extension: "pdf",
        header_bytes: b"%PDF",
        footer_bytes: Some(b"%%EOF"),
        max_size_bytes: 100 * 1024 * 1024,
    },
];
