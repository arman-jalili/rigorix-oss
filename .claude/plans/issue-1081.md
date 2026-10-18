# Issue #1081 — deterministic precondition runner tests

**Branch:** `issue-1081`

## The flake
Three tests spawned `/bin/sh` on a wall-clock budget. The must-pass ones used the
domain **default 5s** while thousands of tests competed for CPU; the timeout case
burned a real 100ms plus a live `sleep` child.

## Fix
| test | before | after |
|---|---|---|
| `argv_exit_codes_are_mapped` | 5s default | explicit 60s (domain max) — the subject is exit-code mapping, not machine speed |
| `json_stdin_and_env_are_provided` | 5s default | explicit 60s |
| `timeout_and_spawn_failure_refuse_never_pass` | real 100ms + `sleep 5` | split: `timeout_refuses_never_passes` on a **virtual clock** (`start_paused`, child `sleep 3600`), `spawn_failure_refuses_never_passes` (deterministic), |
| + new | — | `a_check_inside_its_budget_is_judged_by_its_exit_code` — fault-injection pair |

Same treatment for `runner.rs`'s own `timeout_maps_to_refuse_error`.

## Falsification (load-bearing)
Injected a wrong error variant in the `Err(_elapsed)` arm → **both** timeout tests
failed (and failed instantly, proving the virtual clock does not hang). Reverted.

## Stability
Precondition suite: 72 tests, 0.02s (was dominated by real sleeps), 3 consecutive
clean runs; full lib suite 2231 passed × 2.
