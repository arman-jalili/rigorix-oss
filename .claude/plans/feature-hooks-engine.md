# Batch Plan — feature/hooks-engine

**Source:** `.claude/plans/issue-groups.md` batch 2
**Issues:** #721 GAP-A-04, #722 GAP-A-05
**Files:** `engine/src/hooks/application/hook_runner_impl.rs`, `engine/src/execution_engine/application/service_impl.rs`, tests

## Implementation order

| # | Issue | Change | Risk |
|---|-------|--------|------|
| 1 | #721 A-04 hook-runner deadlock | `execute_single_command`: drain stdout/stderr on reader threads while polling; replace `wait_with_output`-after-reap with thread join; reap after kill on timeout/abort | LOW (internal, only impl of trait) |
| 2 | #722 A-05 hooks in parallel dispatch | `spawn_concurrent_node`: rename `_hook_runner` → used; PreToolUse gating before dispatch (mirrors `execute_tool`); PostToolUse after dispatch; thread `&self.hook_runner` (call sites already pass it) | MEDIUM (parallel path gains hook gating — intended) |

## Validation

```bash
cargo test -p rigorix-engine --lib hooks execution_engine
cargo clippy -p rigorix-engine -- -D warnings
cargo fmt --check
```

## Commits

1. `fix(hooks): drain stdout/stderr concurrently; reap after kill (#721)`
2. `fix(execution-engine): PreToolUse/PostToolUse hooks in parallel dispatch (#722)`
