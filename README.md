# 💽 Data-Recovery_CLI

![Platform](https://img.shields.io/badge/platform-Windows%2010%20%7C%2011-blue?style=flat-square)
![Language](https://img.shields.io/badge/language-Rust%20%7C%20TypeScript-orange?style=flat-square)
![License](https://img.shields.io/badge/license-Proprietary-red?style=flat-square)
![Status](https://img.shields.io/badge/status-Active%20Development-success?style=flat-square)

> A professional, privacy-first Windows data recovery utility designed for recovering accidentally deleted files (including Shift+Delete and emptied Recycle Bin) from NTFS volumes.

**Core Promise:** Recover your own deleted files when the storage has not been overwritten. 

⚠️ *Note: Recovery is never guaranteed. Success depends on overwrite status, fragmentation, SSD TRIM behavior, filesystem condition, and other factors.*

---

## 📑 Table of Contents
- [Core Principles & Safety Rules](#-core-principles--safety-rules)
- [Architecture](#-architecture)
- [Getting Started](#-getting-started)
- [Usage Guide](#-usage-guide)
- [Testing](#-testing)
- [Development Status](#-development-status)
- [License](#-license)

---

## 🛡️ Core Principles & Safety Rules

1. **Read-Only Scanning:** The scanning engine **never** writes to the source device under any circumstances.
2. **Safe Recovery:** Recovered files are never written back to the source drive by default. Attempting to do so triggers a strong warning.
3. **No Execution:** No automatic execution of recovered files to prevent malware execution.
4. **Privacy First:** 100% offline. No telemetry of recovered content and no cloud upload.

## 🏗️ Architecture

The suite is built with a reliable Rust core and a modern web frontend via Tauri, ensuring both high performance and a smooth user experience.

```text
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
```

## 🚀 Getting Started

### Prerequisites
To build and run the project, ensure you have the following installed:
- **Windows 10 or 11 (64-bit)**
- [Rust Toolchain](https://rustup.rs/) (latest stable)
- [Node.js](https://nodejs.org/) (v16+)
- Cargo & npm

### Building the Project

The workspace includes both the CLI engine and the Desktop UI.

**1. Core Engine (Rust):**
```bash
# Build the Rust workspace
cargo build --release
```

**2. Desktop UI (Tauri + React):**
```bash
cd apps/desktop
npm install
npm run tauri build
```

## 💻 Usage Guide

### Command Line Interface (CLI)
You can run the recovery engine directly from the CLI. Raw disk access requires administrator privileges.

**To run the tool and recover your files, follow these steps:**
1. Open **Windows PowerShell** as Administrator.
2. Navigate to the project directory: `cd /d D:\TESting\testing\windows-data-recovery`
3. Run the recovery command (replace `\\.\D:` and `C:\RecoveredData` with your source and destination):
   ```powershell
   # Recover specific files using a filter
   target\release\recover-cli.exe recover \\.\D: --output C:\RecoveredData --filter "flerken"
   
   # Or recover EVERYTHING on the drive (use with caution)
   target\release\recover-cli.exe recover \\.\D: --output C:\RecoveredData
   ```

### Desktop UI
To run the graphical interface in development mode with hot-reloading:

```bash
cd apps/desktop
npm run tauri dev
```

*Screens include:* Home (drive selection) → Scan Mode → Progress → Results → Recover → Report.

## 🧪 Testing

The test suite includes controlled recovery fixtures under `tests/fixtures/`. Run the full test suite with:

```bash
cargo test --workspace
```

### Verified Vertical Slice
On the synthetic NTFS test image (`tests/fixtures/synthetic_ntfs.img`), exact SHA-256 recovery is currently verified for:
- `notes.txt` (resident deleted file)
- `payload.bin` (non-resident data runs)

### How to see the tool work perfectly (Live Testing)
To successfully test the tool on your physical machine without interference from Windows or SSD hardware features, follow these steps:
1. Use a **USB Flash Drive** or a non-system **Hard Disk Drive (HDD)**. *(Do not use your `C:` drive, and avoid modern SSDs with TRIM enabled).*
2. Create a test folder (e.g., `flerkenstudio`) on the drive.
3. Place a sample file (e.g., a PDF or JPG) inside the folder.
4. Permanently delete the folder (using `Shift + Delete`).
5. Run the CLI recovery command immediately:
   ```powershell
   target\release\recover-cli.exe recover \\.\D: --output C:\RecoveredData --filter "flerkenstudio"
   ```
   *Note: Ensure your `--output` folder is on a different drive than your test drive!*

**Sample Test Result:**
```text
D:\TESting\testing\windows-data-recovery>target\release\recover-cli.exe recover \\.\D: --output C:\RecoveredData --filter "flerken_test"
Opening device/image: \\.\D:...
Scanning MFT records...
[|] Scanned 1600512 / 1600512 MFT records (100%) | Found 899753 deleted candidate file(s)...
Recovering 3 candidate file(s) to "C:\\RecoveredData"...
[|] Recovered       3 /       3 files (100%) | Success: 3, Failed: 0

--- Recovery Summary ---
Total requested : 3
Successful      : 3
Failed          : 0
Destination     : C:\RecoveredData

Details:
  [OK] flerkenstudio (0 bytes) -> SHA-256: e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
  [OK] Honey Mesh Vishwakarma.pdf (85040 bytes) -> SHA-256: b35bc96922454fbba532703f48b3b5cbe1a6ae12b4f7304e853e60a89c43d93c
  [OK] HTU8eCGWMAADqQ6.jpg (1102096 bytes) -> SHA-256: 18168fd5528405934656b653c63ea056f77fc14d292b34988094238ba9adf23c
```

### Testing on Live Drives & SSD TRIM Limitations
When testing the recovery tool on a live, physical Windows drive, be aware of the following:

**1. The "Active C: Drive" Overwrite Limitation:**
If you delete a file on your primary system drive (`C:`), Windows background processes (telemetry, indexing, logging) will instantly reuse the freed clusters. This destroys the file's MFT record and overwrites the physical data. **Always test on a secondary drive (e.g., `D:`) or a USB flash drive.**

**2. Modern SSD Hardware TRIM (DZAT):**
When a file is deleted on a modern Solid State Drive (NVMe or SATA), Windows immediately issues a **TRIM** hardware command to the SSD controller. The controller unmaps the physical flash blocks. If a data recovery tool reads the raw LCNs of those blocks, the SSD controller intercepts the request and guarantees a response of **100% zeroes** (Deterministic Read Zero after TRIM). 
*Note: Because of TRIM, deleted files on modern SSDs are permanently destroyed in milliseconds. The tool will successfully find the deleted MFT records and rebuild the file paths, but the extracted files will contain only zeroes. To test actual data extraction, use an HDD, a USB drive, or an SSD with TRIM explicitly disabled.*

## 📈 Development Status

Current milestone focus: **Phase 1 — NTFS Read-Only Scanner + Recovery vertical slice**.
See `docs/architecture/` for detailed phase planning.

## 📄 License
Proprietary / TBD. Intended solely for legitimate recovery of data the user is authorized to access.
