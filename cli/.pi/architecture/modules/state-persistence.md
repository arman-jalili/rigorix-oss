# State Persistence

## Module Status

**Status:** Engine contract frozen — CLI uses as library
**Last reviewed:** 2026-06-16
**Source session:** 71e2b81a-a7a1-48ee-ab8f-56284bbec92d

## Description

Persists execution state to disk using atomic write-rename for crash safety. Tracks overall execution status (Pending, Running, Completed, Failed, Cancelled) and per-node state. Supports TUI graph persistence for viewing past executions.

The CLI reads persisted state for `rigorix history` and `rigorix logs` commands.

## Components

**CLI-facing:**
| Component | File (planned) | Module | Purpose |
|-----------|---------------|--------|---------|
| HistoryCommandHandler (trait) | `cli/src/state_persistence/infrastructure/service.rs` | state_persistence | Service trait for history command |
| HistoryEngineHandler | `cli/src/state_persistence/infrastructure/history_handler_impl.rs` | state_persistence | Implements HistoryCommandService via engine StatePersistenceService |
| LogsCommandHandler (trait) | `cli/src/state_persistence/infrastructure/logs_service.rs` | state_persistence | Service trait for logs command |
| LogsEngineHandler | `cli/src/state_persistence/infrastructure/logs_handler_impl.rs` | state_persistence | Implements LogsCommandService via engine EventSystemService |

**Engine dependencies (frozen contracts):**
| Component | Engine Source | Contract |
|-----------|--------------|----------|
| ExecutionState (aggregate root) | `engine/src/state_persistence/domain/state.rs` | Serializable execution snapshot |
| NodeState | `engine/src/state_persistence/domain/state.rs` | Per-node state: status, output, retries |
| ExecutionGraph | `engine/src/state_persistence/domain/graph.rs` | Graph structure for TUI history |
| StateManager | `engine/src/state_persistence/application/` | Atomic persistence with file locking |
| GraphManager | `engine/src/state_persistence/application/` | Persistence for ExecutionGraph records |
| ExecutionRecord | `engine/src/state_persistence/domain/context.rs` | Complete execution record |
| StateError | `engine/src/state_persistence/domain/error.rs` | Typed error enum |

## Domain Events

| Event | Description | Triggered By |
|-------|-------------|-------------|
| StatePersisted | Execution state was atomically written to disk | StateManager |

## Ubiquitous Language

| Term | Definition |
|------|-----------|
| ExecutionState | Serializable snapshot of execution: overall status, per-node states, timing. |
| NodeState | Persisted per-node state: status, retry count, output, error. |
| ExecutionGraph | Serializable graph structure for TUI history view. |
| AtomicWriteRename | Crash-safe file write: write to temp file, fsync, rename over target. |

## Dependencies

- Depends on: `engine::state_persistence` (all contracts frozen)
- Depends on: `Event System` (reads PersistedEvents for ExecutionRecord)
- Used by: `CLI Boundary` (history, logs commands)
