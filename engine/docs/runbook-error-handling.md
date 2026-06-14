# Runbook: error-handling Module

<!--
Canonical Reference: .pi/architecture/modules/error-handling.md
Last Updated: 2026-06-14
-->

## Overview

The `error-handling` module provides structured error types using `thiserror`
across all modules. The root `CoreOrchestratorError` aggregates all domain-specific
errors via `#[from]` for consistent error propagation and Display chains.

**Components:**
- `CoreOrchestratorError` (`src/error.rs`) — root error enum with 15 domain sub-errors
- `ExecutionError` (`src/execution/domain/error.rs`) — task execution failures

## Startup Sequence

### Dependencies

| Dependency | Required | Description |
|------------|----------|-------------|
| thiserror | Yes | Derive macros for error enums |
| serde_json | Yes | JSON error serialization |
| std::error::Error | Yes | Standard error trait |

### Initialization

Error types are zero-cost abstractions — no initialization required. Error enums
are created on-demand when failures occur. The `CoreOrchestratorError` enum is
constructed automatically via `#[from]` conversions when using the `?` operator:

```rust
use rigorix::error::CoreOrchestratorError;
use rigorix::dag_engine::domain::DagError;

fn my_function() -> Result<(), CoreOrchestratorError> {
    // DagError is automatically converted to CoreOrchestratorError::Dag via #[from]
    let graph = dag_operation()?;
    Ok(())
}
```

### Quick Start

```rust
use rigorix::error::CoreOrchestratorError;
use rigorix::dag_engine::domain::DagError;
use rigorix::execution::domain::ExecutionError;

// Create errors directly
let dag_err = CoreOrchestratorError::Dag(
    DagError::CycleDetected { found: 3, total: 10 }
);

let exec_err = CoreOrchestratorError::Execution(
    ExecutionError::Timeout {
        task_id: "task-1".to_string(),
        timeout_secs: 30,
        elapsed_secs: 35,
    }
);

// Check error properties
println!("Code: {}", dag_err.error_code());    // "DAG_ERROR"
println!("Status: {}", dag_err.http_status()); // 500
println!("Retriable: {}", dag_err.is_retriable()); // false (DAG errors are not transient)
```

## Graceful Shutdown

Error types require no shutdown — they are plain enums with no runtime state.
They are freed when they go out of scope.

## Common Failure Modes

| Failure | Error | Diagnosis | Recovery |
|---------|-------|-----------|----------|
| DAG cycle detected | `CoreOrchestratorError::Dag(DagError::CycleDetected)` | Topological sort failure | Fix graph dependencies to remove cycle |
| Planning budget exhausted | `CoreOrchestratorError::Budget(LlmBudgetError::MaxCallsExceeded)` | LLM call limit reached | Wait for budget reset or increase limits |
| Enforcement limit | `CoreOrchestratorError::Enforcement(EnforcementError::ExecutionLimitReached)` | Tool call limit exceeded | Wait for limit reset or increase limits |
| Task execution timeout | `CoreOrchestratorError::Execution(ExecutionError::Timeout)` | Task exceeded timeout | Increase timeout or optimize task |
| I/O error | `CoreOrchestratorError::Io(io::Error)` | File system or network failure | Check disk space, permissions, network |
| HTTP error | `CoreOrchestratorError::Http { status, url, message }` | Backend returned error | Check backend availability and status codes |
| Operation cancelled | `CoreOrchestratorError::Cancelled(reason)` | Shutdown or user cancellation | Restart operation if needed |
| Audit backend down | `CoreOrchestratorError::Audit(AuditError::SendFailed)` | Backend unreachable | Check audit backend + network |

## Operations

### Check error rate
```bash
# Via observability metrics (if configured)
curl http://localhost:8080/metrics | grep rigorix_errors_total
```

### Debug error propagation
```rust
// Use error_code() for machine-readable logging
error!("Operation failed: {} (code: {}, status: {})",
    err, err.error_code(), err.http_status());

// Use is_retriable() for retry decisions
if err.is_retriable() {
    retry_operation().await?;
} else {
    return Err(err);
}
```

## Configuration Reference

The error-handling module has no configuration of its own. It is a passive
infrastructure that all other modules depend on. Error behavior is configured
per-module (e.g., retry strategies, timeout values).

## Key Environment Variables

| Variable | Purpose | Related Error |
|----------|---------|---------------|
| (none) | Error handling is configured per-module | — |

## Anti-Patterns

```rust
// ❌ NEVER use anyhow in library code
use anyhow::Result;       // BAD

// ✅ Use thiserror for library errors
use thiserror::Error;     // GOOD

// ❌ NEVER unwrap() in production code
let value = result.unwrap();  // BAD

// ✅ Proper error handling with ?
let value = result?;          // GOOD
```

## Related Documentation

| Document | Description |
|----------|-------------|
| `.pi/architecture/modules/error-handling.md` | Architecture module doc |
| `src/error.rs` | CoreOrchestratorError source |
| `src/execution/domain/error.rs` | ExecutionError source |
| `docs/dr-plan-error-handling.md` | Disaster recovery plan |
