# ADR-006: Atomic Write-Rename for State Persistence

**Status:** Accepted
**Date:** 2026-06-13
**Session:** 63c25384-1902-4b72-83bb-257f3f682af5

**Tech Stack:** Rust

## Context

Execution state must persist to disk for crash recovery, TUI history viewing, and audit. The system must guarantee that partial writes from power failures or process crashes do not corrupt persisted state.

## Decision

Use the **write-rename (`.tmp` → final) atomic persistence pattern**.

```rust
// 1. Write to temp file
fs::write(&temp_path, &content)?; // {execution_id}.json.tmp

// 2. Atomic rename
fs::rename(&temp_path, &final_path)?; // → {execution_id}.json
```

On POSIX systems, `rename(2)` is atomic — a power failure during write leaves the original file intact. Cross-process concurrency is managed via `fd-lock` advisory file locking.

## Alternatives Considered

| Alternative | Pros | Cons | Reason Rejected |
|-------------|------|------|-----------------|
| **Write-rename (chosen)** | Atomic on POSIX; no external deps beyond std::fs; simple to audit; crash-safe | Not atomic on all filesystems (NFS) | **Chosen** |
| **SQLite** | ACID transactions; queryable; concurrent-safe | Heavy dependency (500KB+); over-engineered for single-file state; C dependency via libsqlite3 | Rejected — overkill for state snapshots |
| **Direct overwrite** | Simplest code | Power failure corrupts file; partial writes | Rejected — unsafe |
| **Append-only log** | Full history preserved | Complex compaction logic; slow replay | Rejected — not needed for current requirements |

## Consequences

### Positive
- Crash-safe: partial writes never corrupt the last good state
- Simple implementation using std::fs only
- Easy to inspect/debug state files (plain JSON)
- Cross-process locking via fd-lock prevents concurrent corruption

### Negative
- Two write operations per save (write + rename)
- Not atomic on all filesystem types (NFS requires special handling)
- Temporary `.tmp` files may remain on power loss (benign, cleaned on startup)

## Implementation

**Affected Modules:**
- `.pi/architecture/modules/state-persistence.md`

**Files to Update:**
- `rigorix/src/state/persistence.rs` — StateManager with write-rename pattern

---

*Decision date: 2026-06-13*
