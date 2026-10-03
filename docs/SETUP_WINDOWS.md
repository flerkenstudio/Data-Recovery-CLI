# Setup on Windows

## Workspace fix (required for Tauri)

The desktop app lives at `apps/desktop/src-tauri` and **must** be listed in the root workspace.

In `D:\windows-data-recovery\Cargo.toml`, the `[workspace]` `members` array must include:

```toml
members = [
    "crates/recovery-core",
    "crates/ntfs-parser",
    "crates/filesystem",
    "crates/scanner",
    "crates/carving",
    "crates/reconstruction",
    "crates/confidence",
    "crates/signatures",
    "crates/validation",
    "crates/storage",
    "crates/test-utils",
    "crates/recover-cli",
    "apps/desktop/src-tauri",
]
```

If you still have an older zip without that line, add `"apps/desktop/src-tauri",` yourself, save the file, then continue.

## Prerequisites

1. **Rust** — https://rustup.rs — then open a **new** PowerShell  
2. **Node.js 18+** — https://nodejs.org  
3. **Visual Studio Build Tools** with **Desktop development with C++**

```powershell
cargo --version
node --version
```

## Run the GUI

```powershell
cd D:\windows-data-recovery\apps\desktop
npm install
npm run tauri dev
```

First compile can take several minutes.

## Run the CLI (no GUI)

```powershell
cd D:\windows-data-recovery
cargo build -p recover-cli --release
.\target\release\recover-cli.exe scan tests\fixtures\synthetic_ntfs.img --max-records 16
.\target\release\recover-cli.exe recover tests\fixtures\synthetic_ntfs.img --output D:\RecoveredTest --max-records 16
```

## Common errors

| Error | Fix |
|-------|-----|
| `package.json` ENOENT | `cd` into `apps\desktop` first |
| `cargo` program not found | Install Rust; new terminal |
| workspace members error | Add `apps/desktop/src-tauri` to root `Cargo.toml` as above |
| MSVC / link errors | Install C++ Build Tools, reboot |
