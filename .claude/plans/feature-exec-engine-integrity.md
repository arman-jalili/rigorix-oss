# Batch Plan — feature/exec-engine-integrity

**Source:** `.claude/plans/issue-groups.md` batch 1
**Issues:** #718 GAP-A-01, #719 GAP-A-02, #720 GAP-A-03, #750 GAP-H-08
**Files:** `engine/src/execution_engine/application/service_impl.rs`, `dto/mod.rs`, `engine/src/orchestrator/application/orchestrator_impl.rs`, `engine/src/execution_engine/tests.rs`

## Implementation order (dependencies first)

| # | Issue | Change | Risk |
|---|-------|--------|------|
| 1 | #718 A-01 unknown tool fails | `execute_tool` `_ =>` arm (:686-697) and `spawn_concurrent_node` `other =>` arm (:231-241) → `TaskResult::failure(..., "unknown_tool", ...)` | MEDIUM (production behavior change, intended) |
| 2 | #719 A-02 missing node fails | `execute_node` else-branch (:1759-1768) → failure `("node_not_found")` tuple instead of placeholder success | LOW (no runtime callers) |
| 3 | #720 A-03 config_override applied | `execute_graph` passes effective config into `run_dispatch_loop`; loop reads `config.max_concurrent_executions` instead of `self.config` | LOW (callers pass `None`) |
| 4 | #750 H-08 hydrate started_at | Add `started_at` to `HydrateExecutionInput`; orchestrator passes `loaded.state.started_at`; hydrate uses it instead of `Utc::now()` | LOW |

## Validation

```bash
cargo build -p rigorix-engine
cargo test -p rigorix-engine execution_engine
cargo clippy -p rigorix-engine -- -D warnings
cargo fmt --check
```

## Commits (one per issue, `fix: ... #<num>`)

1. `fix: unknown tool must fail instead of reporting success (#718)`
2. `fix: missing node must fail instead of placeholder success (#719)`
3. `fix: apply config_override to the dispatch loop (#720)`
4. `fix: hydrate_execution preserves persisted started_at (#750)`
