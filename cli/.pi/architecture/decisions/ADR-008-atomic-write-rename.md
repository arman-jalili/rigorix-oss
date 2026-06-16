# ADR-008: Atomic Write-Rename for State Persistence

**Status:** Accepted
**Date:** 2026-06-16

## Context

Execution state must survive a process crash. The persistence mechanism must guarantee that the state file is never in a partially-written state.

## Decision

**Use atomic write-rename** for all state file operations:

```
1. Serialize state to JSON string
2. Write to .rigorix/state/<id>.tmp  (new temp file)
3. fsync() the temp file descriptor
4. rename() .tmp → .json             (atomic on POSIX)
```

After `rename()`, the `.json` file is guaranteed to contain the complete state. If the process crashes before `rename()`, only the `.tmp` file is orphaned — the previous `.json` is untouched.

## Crash Recovery

On startup:
1. Scan `.rigorix/state/` for orphan `.tmp` files
2. Delete them (they are incomplete writes)
3. Check for `.json` files from previous runs
4. If found, offer to resume the execution

## Implementation

```rust
pub fn atomic_write(path: &Path, contents: &str) -> Result<(), StateError> {
    let tmp_path = path.with_extension("tmp");
    
    // Write to temp file
    let mut file = std::fs::File::create(&tmp_path)?;
    file.write_all(contents.as_bytes())?;
    
    // fsync to ensure data hits disk
    file.sync_all()?;
    
    // Atomic rename (POSIX guarantee: rename() is atomic on same filesystem)
    std::fs::rename(&tmp_path, path)?;
    
    // fsync parent directory to persist the rename
    if let Some(parent) = path.parent() {
        if let Ok(dir) = std::fs::File::open(parent) {
            dir.sync_all()?;
        }
    }
    
    Ok(())
}
```

## Alternatives

| Alternative | Reason Rejected |
|-------------|----------------|
| Direct write to target file | Partial write on crash = corrupted state |
| Write + sync to target | Still vulnerable to crash during write |
| SQLite | Over-engineering for simple JSON state blobs |
| Write-ahead log | More complex than needed for single-file state |

*Affects: State Persistence*
