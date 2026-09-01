# fix(hooks): pipe deadlock + hook gating in parallel dispatch

**Batch 2** of the gap-ledger implementation backlog (`feature/hooks-engine`).
Closes **#721** (GAP-A-04), **#722** (GAP-A-05).

## #721 — hook-runner subprocess pipe deadlock

`execute_single_command` polled `child.try_wait()` in a 10ms loop **without draining stdout/stderr**, then called `wait_with_output()` after the child was already reaped. A hook writing more than the ~64KB pipe buffer blocked forever on `write(2)` — only the wall-clock timeout escaped.

- stdout/stderr drained concurrently on reader threads while polling
- On exit: join threads (EOF once the child's write ends close)
- On timeout/abort/wait-error: `kill()` → `wait()` (reap) → join → return — no zombies, no leaked pipes
- **Regression test:** 200KB-output hook returns `InvalidJson` promptly instead of the 1s `Timeout`; abort test kills + reaps a long-running hook

## #722 — hooks bypassed in the parallel dispatch loop

`spawn_concurrent_node` received `_hook_runner` (underscore-unused) — the production `execute_graph` path skipped PreToolUse/PostToolUse hooks entirely, while the single-node `execute_tool` path ran them. Documented hook semantics did not apply to graph-parallel execution.

- Renamed `_hook_runner` → used; cloned into the spawned task
- **PreToolUse gating** after the permission check, mirroring `execute_tool` (deny/fail/cancel → `TaskResult::failure("hook_blocked")` + NodeCompleted)
- **PostToolUse** after dispatch (informational, no gating)
- Call sites already passed `&self.hook_runner` (`run_dispatch_loop`)
- **Test:** a deny PreToolUse hook blocks a parallel-path `run_command` tool

The parallel path now runs the full gate: **permission → pre-hook → exec → post-hook** (permission was already enforced in both paths — verified).

## Verification
- 1889 engine lib tests green (+3 new: large-output no-deadlock, abort-kill, parallel hook gate)
- `clippy -D warnings` clean, `fmt --check` clean, workspace builds
