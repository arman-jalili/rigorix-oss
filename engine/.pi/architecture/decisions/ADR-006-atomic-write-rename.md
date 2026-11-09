# ADR-006: Atomic Write-Rename for State Persistence

**Status:** Implemented (state_persistence: {id}.json.tmp -> rename in filesystem_state_repository).
**Amended 2026-11-09:** the handle rule extends to caller-supplied trees — see
[Amendment](#amendment-2026-11-09-the-handle-rule-extends-to-a-tree-with-an-adversary).
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

---

## Amendment (2026-11-09): the handle rule extends to a tree with an adversary

**By:** SINK-2 (#1479), SINK-8 (#1494), SINK-9 (#1493). **See also:** ADR-023 §2.

### What this ADR decided, and what it did not

The decision above is about **engine-owned state**: `state_persistence` writes a file the engine
chose, to a directory the engine owns. The threat model is a crash, so `rename(2)` is the whole
answer — nobody is racing the write.

`tools/infrastructure/fs_at.rs` applies the same *shape* to a different threat model: a
**caller-supplied path inside a tree where another process may be writing**. There the adversary
is not a power failure, it is a component being swapped for a symlink between the check and the
use. Atomicity does not address that at all; the *handle* does.

### The extended rule

1. **Resolve once, mutate through the handle.** Every component is opened with
   `openat(…, O_DIRECTORY|O_NOFOLLOW)` from an opened base; the parent is kept open and the
   mutation is `renameat`/`openat` inside it. A path is never re-resolved after validation.
2. **The check returns a type the mutator consumes.** `ValidatedWritePath`'s fields are private
   and it exposes no `PathBuf`. A caller cannot hold the resolution and call `fs::write`; the
   only way to mutate is to hand the value back to `write_atomic`, `append_to_file`,
   `append_or_create` or `read_file`. Registry-style discipline is not enough — this is the type
   system doing the work (ADR-023 `type`).
3. **Non-unix refuses.** A path-based fallback would reintroduce exactly the re-resolution the
   amendment removes, on the platform where `openat` is unavailable.
4. **The base is canonicalized once, and only the base.** It came from configuration. Everything
   below it is walked with `O_NOFOLLOW`, which is where a caller's string lives (#1493).

### The corollary worth writing down

> **A check and a use in separate events are not a control.**

This is the sentence that would have prevented #1479, #1494 and #1493 at review time, and it is
the one sentence neither this ADR nor the code carried. `fs_at`'s module doc says it now.

### Consequences of the amendment

- `state_persistence`'s repositories are unaffected in behaviour; they are engine-owned and stay
  on the `{id}.json.tmp → rename` shape (now expressed through `fs_at` where they touch a path
  a deployment configured).
- The engine-owned absolute stores (`policy`, symbol cache, hook config) resolve through
  `resolve_absolute_for_write`, whose doc states the limit: its base is derived from the same
  string, so it must not be given a path a caller can name.
