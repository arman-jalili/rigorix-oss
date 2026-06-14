# Runbook: state-persistence Module

<!--
Canonical Reference: .pi/architecture/modules/state-persistence.md
Last Updated: 2026-06-14
-->

## Overview

The `state-persistence` module provides atomic persistence for execution state
using write-rename crash safety. It tracks overall execution status (Pending,
Running, Completed, Failed, Cancelled) and per-node state (Pending, InProgress,
Completed, Failed, Skipped). Supports TUI graph persistence for viewing past
executions.

## Components

| Component | Type | Description |
|-----------|------|-------------|
| `ExecutionState` | Domain entity | Serializable execution snapshot with per-node states |
| `NodeState` | Domain entity | Per-node state with status, output, errors, retries, duration |
| `FileSystemStateManager` | Application service | Atomic state persistence with load/save/node-transitions |
| `FileSystemStateRepository` | Infrastructure | Filesystem-backed state storage with atomic write-rename |
| `FileSystemGraphManager` | Application service | Graph persistence for TUI history view |
| `FileSystemGraphRepository` | Infrastructure | Filesystem-backed graph storage with execution index |
| `FileSystemExecutionRecordRepository` | Infrastructure | Complete execution record storage |
| `FileSystemStateManagerFactory` | Factory | Constructs state managers with configurable storage |

## Startup Sequence

### Dependencies

| Dependency | Required | Description |
|------------|----------|-------------|
| tokio runtime | Yes | Async I/O for file operations |
| serde + serde_json | Yes | State serialization/deserialization |
| chrono | Yes | Timestamps (ISO 8601 UTC) |
| uuid | Yes | Execution and node identifiers |
| indexmap | Yes | Deterministic node state ordering |
| tempfile (dev only) | Yes | Unit test temporary directories |

### Initialization

1. Create a `FileSystemStateRepository` with a state directory path
2. Create a `FileSystemStateManager` wrapping the repository
3. (Optional) Create a `FileSystemGraphRepository` for TUI history
4. Start saving state at execution phases:

```rust
use rigorix::state_persistence::application::*;
use rigorix::state_persistence::domain::*;
use rigorix::state_persistence::infrastructure::*;

// Create state directory and repository
let repo = FileSystemStateRepository::new("/var/lib/rigorix/state").await?;
let manager = FileSystemStateManager::new(Box::new(repo));

// Create graph directory (for TUI history)
let graph_repo = FileSystemGraphRepository::new("/var/lib/rigorix/graphs").await?;
let graph_manager = FileSystemGraphManager::new(Box::new(graph_repo));

// Save state at each phase
let state = ExecutionState::new(execution_id, symbol_graph_hash);
manager.save_state(SaveStateInput { state }).await?;
```

## Configuration Reference

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `RIGORIX_STATE_DIR` | `~/.rigorix/state` | Directory for execution state files |
| `RIGORIX_GRAPH_DIR` | `~/.rigorix/graphs` | Directory for execution graph files |
| `RIGORIX_RECORDS_DIR` | `~/.rigorix/records` | Directory for execution records |
| `RIGORIX_MAX_GRAPHS` | `1000` | Maximum number of graph records to retain |

### State Directory Layout

```
~/.rigorix/
├── state/
│   ├── {execution_id}.json          # Execution state (atomic write-rename)
│   └── {execution_id}.json.tmp      # Temp file (should not exist on clean state)
├── graphs/
│   ├── {graph_id}.graph.json        # Execution graph
│   ├── idx_{execution_id}.graph.json # Execution ID index for graph lookup
│   └── {graph_id}.graph.json.tmp    # Temp file
└── records/
    ├── {record_id}.record.json       # Execution record
    ├── idx_{execution_id}.record.json # Execution ID index for record lookup
    └── {record_id}.record.json.tmp   # Temp file
```

## Graceful Shutdown

### Procedure

1. **Complete current save operation:** Wait for any in-flight `save_state` or
   `save_graph` calls to complete.
2. **Save final state:** If an execution is in progress, save the final
   `ExecutionState` with `ExecutionStatus::Cancelled`.
3. **No explicit cleanup needed:** State files are written atomically — partial
   writes from a crash or SIGKILL leave the previous intact state file.

### Signal Handling

| Signal | Behaviour | State Recovery |
|--------|-----------|----------------|
| SIGTERM | Graceful shutdown | `Cancelled` state saved before exit |
| SIGINT (Ctrl+C) | Interrupt | Partial state preserved (atomic writes guarantee validity) |
| SIGKILL | Immediate termination | Prior state file preserved intact |

## Common Failure Modes and Recovery

### Failure: Corrupted State File

**Symptoms:** `StateManagerService::load_state()` returns
`StateError::CorruptedState`.

**Recovery:**

1. Check if a `.json.tmp` temp file exists (indicates crash during write)
2. If temp file is valid, rename it to replace the corrupted `.json` file
3. If neither file is valid, the state is unrecoverable — start a new execution

### Failure: State Directory Permissions

**Symptoms:** `StateRepository::save()` returns `StateError::IoError` with
"Permission denied".

**Recovery:**

1. Verify the state directory exists and is writable
2. Check directory permissions: `ls -la $(dirname "$RIGORIX_STATE_DIR")`
3. Ensure the running user has write access

### Failure: Disk Full

**Symptoms:** `StateRepository::save()` returns `StateError::IoError`.

**Recovery:**

1. Free disk space by removing old state files:
   ```bash
   rm ~/.rigorix/state/*.json
   rm ~/.rigorix/graphs/*.graph.json
   rm ~/.rigorix/records/*.record.json
   ```
2. Old state files can be archived before deletion

### Failure: Cross-Process Lock Contention

**Symptoms:** State file updates take longer than expected.

**Recovery:**

1. Verify only one orchestrator process is running per state directory
2. Check for orphaned lock files if using fd-lock
3. The atomic write-rename pattern ensures no data corruption even under
   concurrent access — only one writer's state will persist

## Observability

### Metrics

| Metric | Source | Description |
|--------|--------|-------------|
| `state_persistence.saves` | Counter | Total state save operations |
| `state_persistence.loads` | Counter | Total state load operations |
| `state_persistence.save_errors` | Counter | Failed save operations |
| `state_persistence.load_errors` | Counter | Failed load operations |
| `state_persistence.state_count` | Gauge | Number of stored state files |
| `state_persistence.graph_count` | Gauge | Number of stored graph files |
| `state_persistence.save_duration_ms` | Histogram | Save operation latency |

### Health Check

The `/api/v1/state/health` endpoint returns:

```json
{
  "status": "ok",
  "state_count": 42,
  "graph_count": 15,
  "state_dir": "/var/lib/rigorix/state"
}
```

### Structured Logging

Key log events:

| Event | Level | Context |
|-------|-------|---------|
| State saved | INFO | execution_id, status, node_count |
| State loaded | INFO | execution_id |
| Node state updated | DEBUG | execution_id, node_id, from_status → to_status |
| State file corrupted | ERROR | execution_id, path, detail |
| Save operation failed | ERROR | execution_id, error |

---
*Last updated: 2026-06-14*
