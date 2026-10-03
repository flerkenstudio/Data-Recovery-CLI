# Data Recovery CLI

![Platform](https://img.shields.io/badge/platform-Windows%2010%20%7C%2011-blue)
![Language](https://img.shields.io/badge/language-Rust%20%7C%20TypeScript-orange)
![License](https://img.shields.io/badge/license-Proprietary-red)

A professional, privacy-first Windows data recovery utility designed for recovering accidentally deleted files (including Shift+Delete and emptied Recycle Bin) from NTFS volumes. 

**Core Promise:** Recover your own deleted files when the storage has not been overwritten. *Note: Recovery is never guaranteed. Success depends on overwrite status, fragmentation, SSD TRIM behavior, filesystem condition, and other factors.*

---

## 🛡️ Core Principles & Safety Rules

1. **Read-Only Scanning:** The scanning engine **never** writes to the source device.
2. **Safe Recovery:** Recovered files are never written back to the source drive by default. Attempting to do so triggers a strong warning.
3. **No Execution:** No automatic execution of recovered files to prevent malware execution.
4. **Privacy First:** 100% offline. No telemetry of recovered content and no cloud upload.

## 🏗️ Architecture

The suite is built with a reliable Rust core and a modern web frontend via Tauri.

`	ext
┌──────────────────────────────────────────┐
│              Tauri + React UI            │
│        TypeScript + modern frontend      │
└──────────────────────┬───────────────────┘
                       │
                       ▼
┌──────────────────────────────────────────┐
│          Application / IPC Layer         │
│      Commands, events, progress, errors  │
└──────────────────────┬───────────────────┘
                       │
                       ▼
┌──────────────────────────────────────────┐
│             Rust Recovery Core           │
│  Scanner · NTFS · MFT · Carving ·        │
│  Confidence · Validation · Reconstruction│
└──────────────────────┬───────────────────┘
                       │
                       ▼
┌──────────────────────────────────────────┐
│           Windows Storage Layer          │
│      Safe / raw / read-only disk access  │
└──────────────────────────────────────────┘
`

## 🚀 Getting Started

### Prerequisites
- Windows 10 or 11 (64-bit)
- [Rust Toolchain](https://rustup.rs/) (latest stable)
- [Node.js](https://nodejs.org/) (v16+)
- Cargo & npm

### Building the Project

The workspace includes the CLI engine and the Desktop UI.

**Core Engine (Rust):**
`ash
# Build the Rust workspace
cargo build --release
`

**Desktop UI (Tauri + React):**
`ash
cd apps/desktop
npm install
npm run tauri build
`

## 💻 Usage

### Command Line Interface (CLI)
You can run the recovery engine directly from the CLI. This is useful for testing and scripting.

`ash
# Scan a disk image or physical drive
cargo run -p recover-cli -- scan tests/fixtures/synthetic_ntfs.img --max-records 16

# Recover files to an output directory
cargo run -p recover-cli -- recover tests/fixtures/synthetic_ntfs.img --output /tmp/out --max-records 16
`

### Desktop UI
To run the graphical interface in development mode:

`ash
cd apps/desktop
npm run tauri dev
`

*Screens include:* Home (drive selection) → Scan Mode → Progress → Results → Recover → Report.

## 🧪 Testing

The test suite includes controlled recovery fixtures under 	ests/fixtures/. Run the full test suite with:

`ash
cargo test --workspace
`

### Verified Vertical Slice
On the synthetic NTFS test image (	ests/fixtures/synthetic_ntfs.img), exact SHA-256 recovery is verified for:
- 
otes.txt (resident deleted file)
- payload.bin (non-resident data runs)

## 📈 Development Status

Current milestone focus: **Phase 1 — NTFS Read-Only Scanner + Recovery vertical slice**.
See docs/architecture/ for detailed phase planning.

## 📄 License
Proprietary / TBD. Intended solely for legitimate recovery of data the user is authorized to access.
