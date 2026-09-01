# fix(execution-engine): silent-success paths fail; hydrate preserves started_at

**Batch 1** of the gap-ledger implementation backlog (`feature/exec-engine-integrity`).
Closes **#718** (GAP-A-01), **#719** (GAP-A-02), **#720** (GAP-A-03), **#750** (GAP-H-08).

## What changed

### #718 — unknown tool must FAIL (not report success)
Two dispatch sites returned `TaskResult::success` for any unrecognized tool — a typo'd/unsupported node produced a green run that did nothing:
- `execute_tool` single-node arm → now `TaskResult::failure(..., "unknown_tool", ...)`
- `spawn_concurrent_node` parallel arm → same

### #719 — missing graph/node must FAIL (not placeholder)
`execute_node`'s node-not-found branch returned `("execution output placeholder", ...)` success → now a typed failure (`failure_type: "node_not_found"`).

### #720 — `config_override` applied
`execute_graph` cloned the override into `let _config` (discarded). It now threads the effective config into `run_dispatch_loop`, which reads `max_concurrent_executions` from it instead of `self.config`. Production callers pass `None`, so no behavior change — the existing override test's intent is now real.

### #750 — hydrate preserves persisted `started_at`
`hydrate_execution` reset `started_at` to `Utc::now()`, distorting `duration_ms` for resumed (approval-paused) runs. `HydrateExecutionInput` gains `started_at`; the orchestrator passes `loaded.state.started_at`; hydrate uses it. Regression test asserts the hydrated session keeps the original start and reports the pause in the duration.

### Companion (A-01 blast radius): permission catalog now recognizes exec-layer tool names
The permission catalog used policy-layer names (`read_file`, `write_file`) while the engine dispatches exec-layer names (`file_read`, `file_write`, ...). Unknown exec tools fell back to `WorkspaceWrite` — so `file_read` was denied in ReadOnly. With #718, every real read tool would fail in read-only mode. Added `file_read`/`git_read` (ReadOnly) and `file_write`/`file_append`/`file_patch`/`git_stage` (WorkspaceWrite) to `PermissionConfig::default` and `PermissionPolicy::default_with_mode`.

## Tests
- **New:** unknown-tool adversarial tests (parallel + single-node arms), hydrate-started_at regression
- **Updated:** 3 tests that asserted the old placeholder success now drive real tools (`run_command`, `file_read`) and assert the failure semantics; inline-retry-loop tests run against a real tool through an approval-paused session
- 1886 engine lib tests + integration tests green, workspace builds, `clippy -D warnings` clean, `fmt --check` clean

## Pre-existing (not introduced here)
`validate-architecture.sh` layer-structure check fails on main too: the validator expects a flat `src/domain/` layout, but this codebase uses per-module layer folders (`engine/src/<module>/domain|...`). Tracked as a CI-truth follow-up (A-27).
