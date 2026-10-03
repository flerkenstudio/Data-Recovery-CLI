# Architecture — Windows Data Recovery Suite

## Goals

- Correct, safe, testable recovery of deleted files from NTFS (V1).
- Strict separation of UI, orchestration, parsing, and storage access.
- Read-only source access by design.
- Explainable recovery confidence.
- Incremental delivery: vertical slice first, then expand.

## Layered Design

1. **UI Layer** (Tauri + React + TypeScript)  
   Drive selection, scan modes, progress, results, preview, recovery wizard, reports.  
   No recovery logic lives here.

2. **IPC / Application Layer** (Tauri commands + events)  
   Explicit commands (`list_drives`, `start_scan`, `recover_files`, …) and events (`scan_progress`, `candidate_found`, …).  
   Frontend never receives raw unsafe disk handles.

3. **Recovery Core** (`recovery-core` + supporting crates)  
   Scan lifecycle, candidate indexing, confidence scoring, recovery orchestration.

4. **Filesystem & Parser Layer** (`ntfs-parser`, `filesystem`)  
   Defensive parsing of boot sector, MFT, attributes, runlists, directory relationships.

5. **Storage Layer** (`storage`)  
   Windows volume / physical disk open in read-only mode, sector-aligned reads, device enumeration.

## Data Flow (Scan)

```
Select drive
  → Analyze (filesystem, media type, TRIM hints)
  → Choose mode (Quick / Deep)
  → Start scan (async, cancellable)
  → Read-only sector / MFT reads
  → Discover candidates (metadata + optional carving)
  → Score confidence
  → Index results
  → UI shows filtered / searchable list
```

## Data Flow (Recovery)

```
User selects candidates + destination
  → Validate destination safety (different physical disk preferred)
  → Reconstruct from data runs / carved ranges
  → Write to destination only
  → Optional SHA-256 + structure validation
  → Recovery report
```

## Key Data Models (conceptual)

- `DriveInfo` — letter, size, filesystem, media type, TRIM state, recovery expectation note
- `ScanSession` — id, mode, progress, status, warnings
- `CandidateFile` — id, name, path, size, timestamps, method (Metadata / Carved), confidence, data runs / ranges
- `RecoveryRequest` / `RecoveryResult` — selected files, destination, per-file outcomes, hashes
- `RecoveryConfidence` — score 0–100, status enum, explainable reasons

## Error Philosophy

- Categories: INFO / WARNING / RECOVERABLE / FATAL
- Parser never panics on corrupt data; returns structured errors
- Scan continues past individual bad records when possible
- Cancellation is first-class

## Performance Constraints

- Streaming / windowed reads
- Worker pools with backpressure
- Bounded result buffers
- Efficient indexing for large result sets (hundreds of thousands – millions of candidates)

## Security & Privacy

- Read-only source
- Path sanitization, no traversal
- No auto-execution of recovered content
- No content telemetry
- Admin elevation requested only when required and explained

## Phased Delivery

| Phase | Focus                                      | Success Criterion                                      |
|-------|--------------------------------------------|--------------------------------------------------------|
| 0     | Architecture, models, IPC contract, tests  | Documents + empty crates + workspace buildable          |
| 1     | NTFS read-only scanner                     | Detect deleted records + accurate metadata on test disk|
| 2     | Recovery from data runs                    | Exact byte recovery + SHA-256 match on fixtures        |
| 3     | File carving                               | Signature-based candidates + basic validation          |
| 4     | UI                                         | Full scan → results → recover flow                     |
| 5     | Hardening                                  | Perf, disconnect handling, installer, docs             |

## Current Focus

Implement the smallest vertical slice end-to-end in the engine (still independent of full UI):

```
Drive → NTFS → Read-only → MFT → Deleted record → Metadata → Data runs → Recover → SHA-256
```
