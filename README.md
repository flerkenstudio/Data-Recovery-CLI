# Windows Data Recovery Suite

Professional, privacy-first Windows data recovery utility for recovering accidentally deleted files (including Shift+Delete and emptied Recycle Bin) from NTFS volumes when the underlying data remains recoverable.

**Core promise:** Recover your own deleted files when the storage has not been overwritten.

Recovery is **never guaranteed**. Success depends on overwrite status, fragmentation, SSD TRIM behavior, filesystem condition, and other factors.

## Priority Order

1. Correctness  
2. Data safety (read-only source scanning)  
3. Read-only source-disk behavior  
4. Recovery reliability  
5. Performance  
6. Clear UX  
7. Maintainability  
8. Security  
9. Extensibility  

## Architecture Overview

```
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

- **Rust** recovery engine (independently testable)
- **Tauri** desktop shell
- **React + TypeScript** UI
- Source disk access is **read-only by design**

## Supported Platforms (V1)

- Windows 10 / 11 (64-bit)
- Filesystem priority: **NTFS** first

## Safety Rules

- Scanning never writes to the source device
- Recovered files are never written to the source drive by default
- Same-physical-disk destination triggers a strong warning / confirmation
- No automatic execution of recovered files
- No telemetry of recovered content
- No cloud upload

## Development Status

See `docs/architecture/` and `AGENTS.md`.

Current milestone focus: **Phase 1 — NTFS Read-Only Scanner + Recovery vertical slice**.

## Building

```bash
# Workspace
cargo build

# Desktop (when Tauri app is fully scaffolded)
cd apps/desktop
npm install
npm run tauri dev
```

## Testing

```bash
cargo test --workspace
```

Controlled recovery fixtures live under `tests/fixtures/`.

## License

Proprietary / TBD. Intended for legitimate recovery of data the user is authorized to access.


## CLI (engine)

```bash
cargo run -p recover-cli -- scan tests/fixtures/synthetic_ntfs.img --max-records 16
cargo run -p recover-cli -- recover tests/fixtures/synthetic_ntfs.img --output /tmp/out --max-records 16
```

Exact SHA-256 recovery is verified against `tests/fixtures/synthetic_ntfs.json`.

## Desktop UI (Tauri + React)

```bash
cd apps/desktop
npm install
npm run tauri dev
```

Screens: Home (drives / open image) → Scan mode → Progress → Results → Recover → Report.

IPC commands: `list_drives`, `open_image`, `start_scan`, `cancel_scan`, `get_candidates`, `recover_files`.

Deep / Advanced scan modes are explicitly marked **NOT IMPLEMENTED**.

## Verified vertical slice

On the synthetic NTFS image:

| File | Kind | Result |
|------|------|--------|
| notes.txt | resident deleted | exact SHA-256 |
| payload.bin | non-resident data runs | exact SHA-256 |
