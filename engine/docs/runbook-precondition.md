# Runbook: precondition Module

<!--
Canonical Reference: .pi/architecture/modules/precondition.md
ADR: .pi/architecture/decisions/ADR-017-consequence-gating.md
Last Updated: 2026-10-02
-->

## Overview

The `precondition` module (`engine/src/precondition/`) makes a
consequential step's dispatch depend on the
**present standing of its authority**, revalidated at the moment of consequence
(T₀ → ΔN → Tₙ), and makes a failed step stop what depends on it. It is a small,
deterministic, fail-closed primitive:

> **match a step → run an operator-authored check → refuse on failure → record
> the outcome.**

It does **not** close the gap between the check and the action; it narrows it,
refuses on a negative answer, and preserves the determination in the signed
envelope.

Three behaviors:

1. **R1 dispatch-time preconditions** — operator-authored
   `.rigorix/preconditions.toml`: a `StepPredicate` match + `require_params` +
   a deterministic argv check (JSON stdin; exit 0 = authority stands). Evaluated
   at the ADR-011 choke point (after approval verification, before
   `spawn_concurrent_node`). Refuse halts the node before its tool is called.
2. **R2 step-outcome gating** — `[gating].release_dependents_on_failure`
   (default `true` = today's behavior). When `false`, a failed/denied step's
   transitive dependents are marked `Skipped` and never dispatched.
3. **R3 required-companion-step** — `require_companion_step` on operator
   requirements: a matched step requires the plan to contain a companion step.

## Components

| Component | Type | Description |
|-----------|------|-------------|
| `Precondition` / `FailureAction` | Domain aggregate | `id`, `match` (reused `StepPredicate`), `require_params`, argv `command`, `timeout_ms` (default 5000), `failure` (`deny`), `capture_output` (default false) |
| `PreconditionConfig` / `SafetyCaps` | Domain config | `fail_closed` (default true) + `[[preconditions]]` + `[gating]`; caps: 100 preconditions / 32 argv / 60s timeout / 16 required params |
| `GatingMode` | Domain value | `release_dependents_on_failure` (default true) |
| `PreconditionOutcome` / `PreconditionFinding` / `PreconditionChecked` | Domain value | `passed` / `failed` / `error`; envelope finding + event payload (no values, no stdout) |
| `PreconditionVerdict` | Domain value | `Dispatch` / `Deny { precondition_id, outcome }` |
| `PreconditionError` | Domain error | 7 variants, `Display`, `error_code()`, all **non-retriable** |
| `PreconditionService` / `PreconditionServiceImpl` | Application trait/impl | match → `require_params` → run → verdict + redacted findings |
| `DispatchGate` / `PreconditionDispatchGate` | Application trait/impl | choke-point assess + fail-closed arming (`NotArmed`) |
| `CompanionStepObligationService` / `…Impl` | Application trait/impl | R3 plan-time companion evaluation |
| `PreconditionSurfaces` / `PreconditionSurfacesImpl` | Application surfaces | structured `policy_violation` + redacted `precondition_findings[]` |
| `HardeningConfig` | Application config | `max_failures_before_abort` from `rigorix.toml` |
| `PreconditionRepository` / `TomlPreconditionRepository` | Infra trait/impl | reads `.rigorix/preconditions.toml`; missing → `Ok(None)`; corrupt/over-cap → `Err` |
| `PreconditionRunner` / `ProcessPreconditionRunner` | Infra trait/impl | argv only, JSON stdin, env, wall-clock timeout, trust boundary |

## Dependencies

| Dependency | Purpose | Failure behavior |
|------------|---------|------------------|
| `.rigorix/preconditions.toml` | Operator check config (same trust surface as permissions/policy) | **Missing** → no gating (fail-open-absent); **corrupt / over cap** → `Err` → matching step refused (fail closed) |
| Operator check program (`command[0]`) | The external authority check | Must resolve **outside the agent-writable workspace**; inside → `TrustBoundary` refusal. Non-zero exit → `failed`; timeout/spawn → `error`; both refuse |
| Approval module (ADR-011) | Precedes the precondition gate at the choke point | Unchanged; the precondition gate runs only after `verify_before_dispatch` passes |
| Sequence policy (ADR-013) | Runs after the precondition gate | Unchanged |
| Audit envelope / event system | `PreconditionChecked` event + `precondition_findings[]` evidence | Event publish failures are warn-logged, never silent |
| Permission enforcer (ADR-013 R5) | `.rigorix/**` agent-write denial | The config is agent-write-protected; the command program is additionally trust-boundary checked |

## Startup Sequence

1. **No module startup is required** — the module is stateless between runs.
   The config is read per dispatch through the injected repository; the answer
   is never cached across T₀→Tₙ.
2. **Wiring** — composition roots call
   `PreconditionSetup::from_env(repo_root)`, which loads
   `<repo>/.rigorix/preconditions.toml`, builds a `PreconditionServiceImpl` over
   a `TomlPreconditionRepository` + `ProcessPreconditionRunner`, wraps it in a
   `PreconditionDispatchGate` (armed; `PreconditionSetup::unarmed(...)` when the
   config is malformed — fail closed), and reports the parsed `[gating]` mode.
   The factory attaches both via
   `ParallelExecutionServiceImpl::with_precondition_gate(gate)` and
   `with_gating_mode(mode)`.
3. **Gating mode** — attach `with_gating_mode(GatingMode { release_dependents_on_failure })`
   from the config `[gating]` table.
4. **Hardening** — `max_failures_before_abort` is read from `rigorix.toml`
   (`Config.max_failures_before_abort`) and threaded into
   `ParallelExecutorConfig` by the composition root.
5. **Health/observability** — the engine's health service and Prometheus
   metrics cover the executor; the module emits `tracing` diagnostics (debug on
   indeterminate checks, warn on unarmed refusals).

## Enabling Preconditions at the Composition Roots

R1/R2 are wired into every supported execution composition root; creating
`.rigorix/preconditions.toml` is the operator's switch:

| Root | File | Behavior |
|------|------|----------|
| MCP host / `rigorix-server` | `mcp/src/host/mod.rs` (`build_real_engine`, reached via `init_host`) | `PreconditionSetup::from_env(repo_root)`; the server inherits the wiring through `rigorix_mcp::host::init_host` |
| CLI | `cli/src/cli_boundary/orchestrator.rs` | `PreconditionSetup::from_env(resolved project root)` |
| GitHub Action | `actions/src/main.rs` | `PreconditionSetup::from_env(repo_root)` (same operator file; no `rigorix.toml` dependency) |

- **File absent** → no gate is attached; behavior is unchanged (fail-open-absent).
- **File valid** → an armed gate plus the `[gating]` mode are attached to the executor.
- **File malformed / over-cap** → the root arms `PreconditionSetup::unarmed(...)`:
  every assessed step is refused (`NotArmed`, fail closed) while the process keeps
  running, so a broken operator file can never silently downgrade to no gating.

To enable: create `.rigorix/preconditions.toml` (see the reference below), make
sure the check program lives outside the agent-writable workspace, and re-run.
No environment variable is required.

## Graceful Shutdown

The module holds no cross-run state, timers, or background tasks — shutdown is
immediate:

1. A check process in flight is spawned with `kill_on_drop(true)`; dropping the
   dispatch future (cancellation) terminates it.
2. No module-owned checkpoint, lock, or drain is required. A paused/aborted run
   resumes via the standard persisted-state path; preconditions are re-read
   fresh at the next dispatch.
3. Emitted `PreconditionChecked` events are drained by the existing event-bus
   path into the envelope.

## Common Failure Modes and Recovery

| Mode | Symptom | Recovery |
|------|---------|----------|
| **Corrupt / over-cap config** | Matching step refused (`ConfigInvalid`, fail closed) | Operator fixes `.rigorix/preconditions.toml`; config errors are non-retriable |
| **Missing config file** | No gating at all (fail-open-absent, documented) | Create the file if gating is expected — a missing file is **not** an error |
| **Configured-but-unarmed** | Step refused with `unarmed` finding (`NotArmed`) | Fix the arming failure (missing executable / malformed entry); never a silent downgrade |
| **Check exits non-zero** | `failed` outcome; tool never called; `precondition_denied` node failure | The authority denied — do not retry; re-establish authority and re-run |
| **Check timeout / spawn error** | `error` outcome; tool never called | Check the check program's availability/latency; raise `timeout_ms` within the cap if legitimate. Fail-closed by design |
| **Command inside workspace** | `TrustBoundary` refusal | Move the check program outside the agent-writable workspace |
| **Missing `require_params`** | Refused **before** the command runs | Ensure the step declares the required canonical pointers |
| **Flaky/slow check blocks work** | Repeated refusals | Keep checks fast/reliable; set `timeout_ms` deliberately (no allow-on-error mode exists) |
| **R2 dependents skipped** | Failed node's dependents `Skipped`, never dispatched | Intended when `release_dependents_on_failure = false`; set `true` to restore release |
| **Agent edits the check** | (Prevented) | `.rigorix/**` writes denied (ADR-013 R5); the command program is trust-boundary checked |
| **Secret leakage into evidence** | (Prevented) | Only `inputs_hash` is recorded; no parameter values or stdout by default (SpanPrivacy) |

## Configuration Reference

```toml
# .rigorix/preconditions.toml — operator-authored. Never agent-writable (R5).
[[preconditions]]
id = "beneficiary-eligible"
match = { tool = "payment_execute" }              # reused StepPredicate
require_params = ["/beneficiary", "/amount"]      # presence-only, else deny
command = ["/opt/rigorix/checks/beneficiary-eligible"]  # argv, no shell, outside the workspace
timeout_ms = 5000                                  # default 5000; cap 60000
failure = "deny"                                   # only v1 action
capture_output = false                             # default: never record stdout

[gating]
release_dependents_on_failure = false              # default true = today's behavior
```

```toml
# rigorix.toml
max_failures_before_abort = 3                       # optional; absent/0 = unlimited
```

**Check command contract:** stdin `{ "precondition_id", "execution_id", "step",
"tool", "parameters" }`; env `RIGORIX_PRECONDITION_ID`, `RIGORIX_EXECUTION_ID`,
`RIGORIX_STEP_NAME`, `RIGORIX_TOOL`; exit 0 → `passed`, non-zero → `failed`,
timeout/spawn/trust-boundary → `error`.

**Safety caps (fixed defaults):** `max_preconditions_per_file = 100`,
`max_argv_args = 32`, `max_timeout_ms = 60000`, `max_require_params = 16`.

## Honest Boundary

A precondition **narrows** the window between the check and the consequence; it
does **not** close it. There remains a gap between the check at T_check and the
action at T_action. For money-out, the downstream system must also enforce
eligibility at its own commit point. Rigorix guarantees: **when the operator's
check says no, the step is refused, and the determination is signed.**

## Escalation

- **False denial / over-blocking**: verify the check program's behavior for the
  legitimate case; adjust `match`/`require_params` or the check itself. The gate
  is deterministic — same step + config + process ⇒ same verdict.
- **Check doesn't fire when it should**: confirm the tool name and JSON pointers
  match the step, and that the config loaded (a missing file is fail-open).
  `rigorix_validate_plan` surfaces findings pre-run.
- **Indeterminate refusals (timeout/spawn)**: treat as an operability signal —
  the check is unavailable; fix availability, do not bypass the gate.
