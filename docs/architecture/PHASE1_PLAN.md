# Phase 1 Implementation Plan — NTFS Read-Only Scanner + Recovery Slice

## Goal

Deliver a **testable vertical slice**:

```
Drive / disk image
  → Read-only open
  → NTFS boot sector
  → MFT walk
  → Deleted FILE records
  → Filename / size / timestamps / data runs
  → Reconstruct file bytes
  → Write to destination (never source)
  → SHA-256 verification
```

## Components (status)

| Component            | Crate            | Status                          |
|----------------------|------------------|---------------------------------|
| Sector reader        | storage          | Implemented (file + Win volume) |
| Drive enumeration    | storage          | Stub (empty list on non-Win)    |
| Boot sector parser   | ntfs-parser      | Implemented + unit tests        |
| Runlist decoder      | ntfs-parser      | Implemented + unit tests        |
| Attribute parser     | ntfs-parser      | Implemented (SI, FN, DATA)      |
| MFT record + USA     | ntfs-parser      | Implemented                     |
| MFT sequential scan  | ntfs-parser      | Implemented (bounded)           |
| Confidence scoring   | confidence       | Implemented                     |
| Quick scan           | scanner          | Implemented                     |
| Reconstruction       | reconstruction   | Implemented (in-memory + stream)|
| Hash validation      | validation       | Implemented                     |
| Orchestration        | recovery-core    | Implemented                     |
| File carving         | carving          | NOT IMPLEMENTED (Phase 3)       |
| Tauri UI             | apps/desktop     | Not started (Phase 4)           |

## Known limitations (Phase 1)

1. **MFT location**: Scanner starts at boot-sector `$MFT` LCN and walks a bounded number of fixed-size records. It does **not** yet follow `$MFT` data runs for very large or fragmented MFTs.
2. **Directory paths**: Parent references are captured; full path reconstruction (walking parent chain) is deferred.
3. **Attribute list**: Multi-record attributes via `$ATTRIBUTE_LIST` not yet expanded.
4. **ADS**: Named data streams ignored for recovery candidates (unnamed `$DATA` only).
5. **Large files**: In-memory reconstruct capped (~512 MB); streaming path exists for later use.
6. **Volume enum**: Windows `list_volumes` is a stub until Win32 APIs are wired.
7. **Admin rights**: Raw volume open on Windows typically requires elevation; errors are explicit.

## Test strategy

### Unit

- Boot sector reject / accept
- Runlist contiguous + sparse + multi-run
- Attribute resident / non-resident bounds checks
- Filename sanitization
- Confidence score bands

### Integration (fixtures)

1. Create controlled NTFS disk image (or use a small VHD) offline.
2. Populate files → Shift+Delete / empty Recycle Bin.
3. Run `quick_scan_ntfs` against the image via `ReadOnlyDevice`.
4. Recover candidates → compare SHA-256 to originals.

### Automated matrix (later)

| FS   | Type | Size  | Delete method   | Fragmented | Method   | Result |
|------|------|-------|-----------------|------------|----------|--------|
| NTFS | TXT  | 4 KB  | Shift+Delete    | No         | Metadata | Exact  |
| NTFS | PDF  | 2 MB  | Empty Recycle   | No         | Metadata | Exact  |
| …    | …    | …     | …               | …          | …        | …      |

## Next steps after Phase 1 stabilizes

1. Follow `$MFT` non-resident runs for complete coverage.
2. Parent-chain path reconstruction.
3. Destination same-disk physical check (Win32).
4. Streaming recovery for multi-GB files.
5. Phase 3 carving + Phase 4 UI.

## Build notes

Requires a normal Rust toolchain (1.75+ recommended; newer preferred).  
The development sandbox used for scaffolding may block Cargo build-script execution; build on a local Windows or Linux machine with standard `cargo`.
