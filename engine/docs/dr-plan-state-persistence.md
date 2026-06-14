# Disaster Recovery Plan: state-persistence Module

<!--
Canonical Reference: .pi/architecture/modules/state-persistence.md
Last Updated: 2026-06-14
-->

## Scope

This DR plan covers the `state-persistence` module — the execution state
persistence system that saves execution snapshots, per-node states, and
execution graphs to disk using atomic write-rename for crash safety.

## RTO/RPO Targets

| Metric | Target | Rationale |
|--------|--------|-----------|
| RTO (Recovery Time Objective) | < 5 minutes | State files on disk can be read immediately on process restart |
| RPO (Recovery Point Objective) | < 1 second | Atomic write-rename ensures the last completed save is always intact |

## Backup Strategy

### What to Back Up

State files are the only persistent data in this module. The directory
structure under `$RIGORIX_STATE_DIR` (default `~/.rigorix/state/`) contains
all persistent state.

| Directory | Contents | Backup Priority |
|-----------|----------|-----------------|
| `state/` | Execution state JSON files (`{uuid}.json`) | High — contains latest execution state |
| `graphs/` | Execution graph JSON files (`{uuid}.graph.json`) | Medium — TUI history, rebuildable |
| `records/` | Execution record JSON files (`{uuid}.record.json`) | Medium — audit trail, rebuildable |

### Backup Schedule

| Type | Frequency | Retention | Method |
|------|-----------|-----------|--------|
| Incremental | Every hour | 24 hours | `rsync` or `cp -u` of new state files |
| Full | Daily | 30 days | Archive `~/.rigorix/` directory |
| Snapshot | Before upgrade | Until next upgrade | Filesystem snapshot (if available) |

### Backup Command

```bash
#!/bin/bash
# Backup script for state-persistence data

BACKUP_DIR="/var/backups/rigorix/state"
TIMESTAMP=$(date +%Y%m%d-%H%M%S)
STATE_DIR="${RIGORIX_STATE_DIR:-$HOME/.rigorix}"

mkdir -p "$BACKUP_DIR/$TIMESTAMP"
rsync -av --link-dest="$BACKUP_DIR/latest" \
  "$STATE_DIR/" \
  "$BACKUP_DIR/$TIMESTAMP/"

# Update latest symlink
rm -f "$BACKUP_DIR/latest"
ln -s "$TIMESTAMP" "$BACKUP_DIR/latest"
```

### Backup Verification

```bash
# Verify backup is readable
for f in "$BACKUP_DIR/$TIMESTAMP"/state/*.json; do
    if ! python3 -c "import json; json.load(open('$f'))" 2>/dev/null; then
        echo "Corrupted backup file: $f"
        exit 1
    fi
done
echo "All state files in backup are valid JSON"
```

## Restore Procedure

### Scenario 1: Process Crash (No Data Loss)

**Impact:** The last completed `save_state()` is intact on disk. Any in-flight
save that was interrupted will have left a `.json.tmp` file.

**Restore:**

1. Start the orchestrator process
2. List available states: `GET /api/v1/state/executions`
3. For any execution that was in-flight, inspect its status:
   - If `Running`, the previous `save_state()` was successful — no recovery needed
   - If a `.json.tmp` file exists for an execution ID, the write was interrupted
4. Run the tmp file recovery:
   ```bash
   for tmp in ~/.rigorix/state/*.json.tmp; do
       id=$(basename "$tmp" .json.tmp)
       real="$HOME/.rigorix/state/$id.json"
       if [ -f "$real" ]; then
           # Temp file is stale — remove it
           rm "$tmp"
       else
           # Temp file may contain valid data — rename to recover
           if python3 -c "import json; json.load(open('$tmp'))" 2>/dev/null; then
               mv "$tmp" "$real"
           else
               echo "Cannot recover $tmp — corrupted temp file"
               rm "$tmp"
           fi
       fi
   done
   ```

### Scenario 2: Disk Failure

**Impact:** State files on the failed disk are lost.

**Restore:**

1. **If backup exists:**
   ```bash
   RESTORE_FROM="/var/backups/rigorix/state/latest"
   STATE_DIR="${RIGORIX_STATE_DIR:-$HOME/.rigorix}"

   # Stop the orchestrator
   # Restore state files
   cp -a "$RESTORE_FROM/state/" "$STATE_DIR/state/"
   cp -a "$RESTORE_FROM/graphs/" "$STATE_DIR/graphs/"

   # Start the orchestrator
   # Verify state is accessible
   ```
2. **If no backup exists:**
   - Execution state is lost
   - Start new executions — the orchestrator will create new state files
   - Historical execution data in TUI will be unavailable
   - Set up backup schedule to prevent future loss

### Scenario 3: Accidental State Deletion

**Restore:**

1. Check backup for the deleted state files
2. Restore specific files:
   ```bash
   RESTORE_FROM="/var/backups/rigorix/state/latest"
   STATE_DIR="${RIGORIX_STATE_DIR:-$HOME/.rigorix}"

   # Restore a specific execution state
   cp "$RESTORE_FROM/state/$EXECUTION_ID.json" "$STATE_DIR/state/"
   ```
3. If no backup, the state is unrecoverable

### Scenario 4: Corrupted State File

**Symptoms:** `load_state()` returns `StateError::CorruptedState`.

**Restore:**

1. **Check for temp file recovery:**
   ```bash
   STATE_DIR="${RIGORIX_STATE_DIR:-$HOME/.rigorix}"
   id="<execution_id>"

   if [ -f "$STATE_DIR/state/$id.json.tmp" ]; then
       if python3 -c "import json; json.load(open('$STATE_DIR/state/$id.json.tmp'))" 2>/dev/null; then
           mv "$STATE_DIR/state/$id.json.tmp" "$STATE_DIR/state/$id.json"
           echo "Recovered from temp file"
       fi
   fi
   ```

2. **Restore from backup:**
   ```bash
   cp "/var/backups/rigorix/state/latest/state/$id.json" "$STATE_DIR/state/"
   ```

3. **If all recovery fails:** Start a new execution — the corrupted state
   is treated as a lost execution.

## Failover Plan

### Single-Process Architecture

The state-persistence module operates as a single process writing to a local
filesystem. There is no distributed failover in the current architecture.

### Manual Failover Steps

1. **Detect failure:** The orchestrator process crashes or hangs
2. **Start new process:** Launch a new orchestrator instance
3. **Recover execution state:**
   - The new process can discover incomplete executions via
     `StateManagerService::list_executions()`
   - Each incomplete execution (status `Running` or `Pending`) can be
     inspected and either resumed or marked as `Failed`/`Cancelled`
4. **Verify state integrity:**
   - Run `cargo test --lib state_persistence` to verify the module works
   - Check `GET /api/v1/state/health` for basic health status

### High Availability (Future)

| Enhancement | Description | Priority |
|-------------|-------------|----------|
| Shared filesystem (NFS) | Store state on shared filesystem for process migration | Medium |
| Database backend | Store state in PostgreSQL for ACID guarantees and failover | Low |
| State replication | Replicate state to a secondary node for hot standby | Low |

---
*Last updated: 2026-06-14*
