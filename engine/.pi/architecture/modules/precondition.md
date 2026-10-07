# Precondition Architecture

<!--
Canonical Reference: .pi/architecture/modules/precondition.md
Blueprint Source: Guardian Framework v1.2
Generated: NEVER (this is the source)
ADR: .pi/architecture/decisions/ADR-017-consequence-gating.md
Epic: /architect --epic "precondition"

IMPORTANT — Guardian invocation:
  This module belongs to the **engine** crate. The architect resolves
  `.pi/architecture/modules` relative to the invocation cwd, so run:
      cd engine && /architect --epic "precondition"
  When the session cwd is the repo root, the root discovery symlink
  `.pi/architecture/modules/precondition.md` resolves to this file so a
  root invocation also picks up THIS module (not module-template).
  Implementation paths in §Implementation Sequence are repo-root relative.
-->

## Overview

Consequence gating makes a consequential step's dispatch depend on the **present
standing of its authority**, revalidated at the moment of consequence
(T₀ → ΔN → Tₙ), and makes a failed step stop what depends on it. It is a small,
deterministic, fail-closed primitive:

> **match a step → run an operator-authored check → refuse on failure → record
> the outcome.**

It does **not** close the gap between the check and the action (see
§Honest Boundary); it narrows it, refuses on a negative answer, and preserves the
determination in the signed envelope.

## DDD Layers

| Layer | Purpose | Tech |
|-------|---------|------|
| `domain/` | Precondition model, gating mode, errors, safety caps | Zero I/O |
| `application/` | Matching + verdict service; dispatch-gate orchestration | Engine services |
| `infrastructure/` | TOML repository; deterministic argv runner | tokio::process, toml |
| `interfaces/` | MCP/CLI/server surfaces + error taxonomy | MCP tools, JSON-RPC |

**Dependency rule:** `domain → application → infrastructure → interfaces`

## Components by Layer

#### Domain Layer (`domain/`)
| Component | Description | Framework? |
|-----------|-------------|------------|
| Precondition | StepPredicate match, require_params, command, timeout, failure, capture_output, require_immutable_check, authority_path | ❌ No |
| GatingMode | `release_dependents_on_failure` | ❌ No |
| PreconditionError | Typed errors; all non-retriable | ❌ No |

#### Application Layer (`application/`)
| Component | Description | Type |
|-----------|-------------|------|
| PreconditionService | Match a step, run the check, return Dispatch/Deny | Service |
| DispatchGate | Choke-point wiring after ADR-011 verify; fail-closed arming | Service |
| CompanionStepObligation | R3 `require_companion_step` on `[[requirements]]` | Service |

#### Infrastructure Layer (`infrastructure/`)
| Component | Description | Connects to |
|-----------|-------------|-------------|
| PreconditionRepository | Parse/validate `.rigorix/preconditions.toml` | Filesystem |
| PreconditionRunner | argv execution, JSON stdin, env, timeout, exit mapping | Process |

#### Interfaces Layer (`interfaces/`)
| Component | Description | 'use client'? |
|-----------|-------------|---------------|
| PreconditionSurfaces | `rigorix_validate_plan` findings + error taxonomy + version capability | No |
| PreconditionFinding | Envelope `precondition_findings[]` evidence ref | No |
| HardeningConfig | `max_failures_before_abort` reachable from `rigorix.toml`; fail-closed arming | No |

---

## Component Details

### Precondition

status: implemented
depends: none

**Purpose:** Domain model of one operator-authored authority check: `id`, a reused `StepPredicate` match, `require_params` (ADR-015 presence obligation), an argv `command`, `timeout_ms`, `failure`, `capture_output`, `require_immutable_check`, `authority_path`, plus `SafetyCaps` validation.

**DDD Layer:** `domain`

**Implementation File:** `src/precondition/domain/precondition.rs`

**Canonical Reference:** `.pi/architecture/modules/precondition.md#precondition`

**Dependencies:**
- `sequence_policy::domain::StepPredicate` (reused, not forked)

### PreconditionRepository

status: implemented
depends: Precondition

**Purpose:** Load and validate `.rigorix/preconditions.toml`: missing file = `Ok(None)` (fail-open-absent); malformed or over-cap = `Err` (fail closed). Applies count/argv/timeout caps.

**DDD Layer:** `infrastructure`

**Implementation File:** `src/precondition/infrastructure/toml_repository.rs`

**Canonical Reference:** `.pi/architecture/modules/precondition.md#config`

**Dependencies:**
- Precondition

### PreconditionRunner

status: implemented
depends: Precondition

**Purpose:** Execute a precondition's `command` deterministically: argv only (no shell), step parameters as JSON on stdin, documented env vars, wall-clock timeout, exit-code mapping. Timeout and spawn failure map to refuse, never pass. The command path must resolve **outside the agent-writable workspace** (trust boundary — see §Security Considerations).

**DDD Layer:** `infrastructure`

**Implementation File:** `src/precondition/infrastructure/runner.rs`

**Canonical Reference:** `.pi/architecture/modules/precondition.md#command-contract`

**Dependencies:**
- Precondition

### PreconditionService

status: implemented
depends: Precondition, PreconditionRunner

**Purpose:** For the node about to dispatch: match preconditions, enforce `require_params` presence (fail closed), run the check, and return `Dispatch` or `Deny{precondition_id, outcome}`. Deterministic; no LLM; no retries that change the verdict.

**DDD Layer:** `application`

**Implementation File:** `src/precondition/application/service_impl.rs`

**Canonical Reference:** `.pi/architecture/modules/precondition.md#dispatch-integration`

**Dependencies:**
- Precondition
- PreconditionRunner

### DispatchGate

status: implemented
depends: PreconditionService

**Purpose:** Wire the precondition verdict into `run_dispatch_loop` at the single choke point, after ADR-011 approval verification and before `spawn_concurrent_node`. On refuse: deterministic node failure, tool never called, dependents released per `GatingMode`. Enforce fail-closed arming (configured-but-unarmed refuses a matching step).

**DDD Layer:** `application`

**Implementation File:** `src/execution_engine/application/service_impl/dispatch.rs`

**Canonical Reference:** `.pi/architecture/modules/precondition.md#dispatch-integration`

**Dependencies:**
- PreconditionService

### PreconditionFinding

status: implemented
depends: DispatchGate

**Purpose:** Additive envelope `precondition_findings[]` (`precondition_id`, `step`, `outcome`, `exit_code`, `inputs_hash`, `checked_at`, `summary`) derived from a `PreconditionChecked` event. No parameter values or stdout by default (SpanPrivacy).

**DDD Layer:** `interfaces`

**Implementation File:** `src/audit/application/envelope_factory_impl.rs`

**Canonical Reference:** `.pi/architecture/modules/precondition.md#evidence`

**Dependencies:**
- DispatchGate

### GatingMode

status: implemented
depends: DispatchGate

**Purpose:** `[gating].release_dependents_on_failure` (default true = today). When false, a failed/denied step does not release its transitive dependents; they are marked `Skipped` and never dispatched.

**DDD Layer:** `domain`

**Implementation File:** `src/precondition/domain/gating.rs`

**Canonical Reference:** `.pi/architecture/modules/precondition.md#r2`

**Dependencies:**
- DispatchGate

### CompanionStepObligation

status: implemented
depends: Precondition

**Purpose:** Extend ADR-015 `[[requirements]]` with `require_companion_step` — a matched step requires the plan to contain a step matching the companion predicate. Unmet → deny (default) or promote, with `requirement_findings[]`. Closes the ADR-015 non-goal.

**DDD Layer:** `application`

**Implementation File:** `src/sequence_policy/domain/requirement.rs`

**Canonical Reference:** `.pi/architecture/modules/precondition.md#r3`

**Dependencies:**
- Precondition (StepPredicate reuse)

### PreconditionError

status: implemented
depends: none

**Purpose:** Typed error enum (`ConfigInvalid`, `NotArmed`, `Match`, `Spawn`, `Timeout`, `Denied`), `Display`, `is_retriable()` (all false — a denied authority is not retriable).

**DDD Layer:** `domain`

**Implementation File:** `src/precondition/domain/error.rs`

**Canonical Reference:** `.pi/architecture/modules/precondition.md#fail-modes`

**Dependencies:**
- none

### PreconditionSurfaces

status: implemented
depends: DispatchGate, PreconditionFinding

**Purpose:** `rigorix_validate_plan` surfaces precondition findings pre-run; runtime refusal maps to a structured `policy_violation` (`data.precondition_id`, `data.step`, `data.outcome`); `rigorix.system.version` advertises the capability. No new `rigorix.*` method.

**DDD Layer:** `interfaces`

**Implementation File:** `mcp/src/execution_tools/`

**Canonical Reference:** `.pi/architecture/modules/precondition.md#surfaces`

**Dependencies:**
- DispatchGate
- PreconditionFinding

### HardeningConfig

status: implemented
depends: DispatchGate

**Purpose:** Make `max_failures_before_abort` reachable from `rigorix.toml` (currently 0/unlimited and hardcoded at cli/actions) and refuse rather than silently degrade when binding/preconditions cannot arm for a consequential run.

**DDD Layer:** `interfaces`

**Implementation File:** `src/configuration/domain/config.rs`

**Canonical Reference:** `.pi/architecture/modules/precondition.md#fail-modes`

**Dependencies:**
- DispatchGate

---

## Dispatch Integration

`run_dispatch_loop`, for the node about to dispatch, in order:

```
1. ADR-011 approval verification   (verify_before_dispatch)
2. PreconditionService evaluation  (NEW)
3. ADR-013 R3 sequence-policy prefix
4. spawn_concurrent_node
```

A refusal records a deterministic node failure and **never** calls the tool.

## Config

```toml
# .rigorix/preconditions.toml  (operator-owned; agents cannot write .rigorix/**)
[[preconditions]]
id = "beneficiary-eligible"
match = { tool = "payment_execute" }
require_params = ["/beneficiary", "/amount"]
command = ["/opt/rigorix/checks/beneficiary-eligible"]  # must be outside the workspace
timeout_ms = 5000
failure = "deny"
capture_output = false
# Opt-in boundary strength: refuse if the check (or authority_path) is writable
# by the engine's effective UID. Default false (path + hook boundary).
require_immutable_check = false
# Optional: the authority artifact the check consults. Hashed into the signed
# envelope (authority_digest, #987); never its contents.
# authority_path = "/usr/local/lib/rigorix-authority-demo/authority.json"

[gating]
release_dependents_on_failure = false
```

### Command Contract

- **stdin:** `{ "precondition_id", "execution_id", "step", "tool", "parameters" }`
- **env:** `RIGORIX_PRECONDITION_ID`, `RIGORIX_EXECUTION_ID`, `RIGORIX_STEP_NAME`, `RIGORIX_TOOL`
- **exit 0** → Passed; **non-zero** → Failed; **timeout / spawn error** → Error
- **Passed → dispatch. Failed and Error → refuse (fail closed) and are recorded distinctly.**

## Fail Modes

| Situation | Behavior |
|-----------|----------|
| No `preconditions.toml` | Fail-open-absent: today's behavior |
| Malformed / over-cap config | Fail closed: affected runs refused |
| Configured but cannot arm | Fail closed: matching step refused (`unarmed`) |
| Command non-zero | Refuse (`failed`); tool never called |
| Command timeout / spawn error | Refuse (`error`); tool never called |
| Command path inside agent-writable workspace | Refuse (`error`); trust boundary |
| `require_immutable_check=true` and the check/authority is writable by the engine's euid | Refuse (`error`, `Boundary`); fail closed |
| `require_immutable_check=true` and writability metadata is unreadable | Refuse (`error`, `Boundary`); fail closed |
| `require_immutable_check=false` (default) and the boundary is writable | Assessment traced; behavior unchanged |
| Non-matching step | Unaffected |

## Implementation Sequence

> **Status (2026-10-02): implemented.** All 11 components live in
> `engine/src/precondition/` (contract freeze #938 + implementation issues
> ISSUE-CONSEQUENCE-GATING-1…11). The 17 acceptance criteria are covered by
> `engine/tests/unit/precondition/` plus the execution-engine integration
> tests (`gating_mode`, dispatch gate). Operability is documented in
> `engine/docs/runbook-precondition.md` and
> `engine/docs/dr-plan-precondition.md`. CI hardening stage 37
> (`check_precondition_contracts.sh`) enforces the contracts automatically.

1. Contract freeze (rigorix-sdk #39): `rigorix-sdk/schemas/policy.json` (preconditions + `[gating]`) + `schemas/envelope.json` (`precondition_findings[]`) + `schemas/api/errors.json` + a signed fixture verified byte-exact in Rust/Python/TypeScript/Java/Go.
2. Domain: `engine/src/precondition/domain/{precondition,gating,error}.rs` + safety caps and fail-closed config validation.
3. Infrastructure/config: `engine/src/precondition/infrastructure/toml_repository.rs` for `.rigorix/preconditions.toml` (missing ⇒ `Ok(None)`, malformed/over-cap ⇒ `Err`).
4. Infrastructure/runner: `engine/src/precondition/infrastructure/runner.rs` — argv only, JSON stdin, env, wall-clock timeout, and the trust-boundary check that `command[0]` resolves outside the agent-writable workspace.
5. Application: `engine/src/precondition/application/{service,service_impl}.rs` — match step, enforce `require_params` presence, run the check, return Dispatch/Deny.
6. Dispatch gate: `engine/src/execution_engine/application/service_impl/dispatch.rs` (after ADR-011 `verify_before_dispatch`, before `spawn_concurrent_node`), plus `engine/src/execution_engine/application/factory.rs` and the composition roots `cli/src/cli_boundary/orchestrator.rs`, `actions/src/main.rs`, `server/src/**`.
7. Evidence: `engine/src/event_system/domain/event.rs` (`PreconditionChecked`) + `engine/src/audit/domain/envelope.rs` + `engine/src/audit/application/envelope_factory_impl.rs` (SpanPrivacy: no values, no stdout by default).
8. R2 step-outcome gating: `engine/src/precondition/domain/gating.rs` + the release-dependents logic in `engine/src/execution_engine/application/service_impl/dispatch.rs`.
9. R3 companion step: `engine/src/sequence_policy/domain/{requirement,config}.rs` + `engine/src/sequence_policy/application/service_impl.rs` (only when this phase is in scope).
10. Surfaces: `mcp/src/execution_tools/**` + `mcp/src/host/error.rs` + `server/src/version.rs` (no new `rigorix.*` method).
11. Hardening: `engine/src/configuration/domain/config.rs` (`max_failures_before_abort` reachable) + fail-closed arming in `engine/src/execution_engine/application/factory.rs`.
12. Falsifier/demo: `engine/tests/precondition_e2e.rs` + `demo/consequence-gating/run.sh` (T0 approve → ΔN flip → Tn refuse + signed evidence).

## Acceptance Criteria

| # | Component | Criterion | Verify In |
|---|-----------|-----------|-----------|
| 1 | Precondition | TOML parse → precondition with all fields; serde round-trip preserves them | unit test |
| 2 | PreconditionRepository | Malformed/over-cap → `Err` (fail closed); missing file → `Ok(None)` | unit test |
| 3 | PreconditionRunner | argv execution, JSON stdin, env set, exit code mapped; no shell | unit test |
| 4 | PreconditionRunner | A command path resolving inside the agent-writable workspace is refused | unit test |
| 5 | PreconditionRunner | Timeout and spawn failure map to refuse (`error`), never pass | unit test |
| 6 | PreconditionService | Matched step exit 0 → `Dispatch`; non-zero → `Deny` | unit test |
| 7 | PreconditionService | `require_params` absent → `Deny` before the command runs | unit test |
| 8 | PreconditionService | Non-matching step is unaffected (no process spawned) | unit test |
| 9 | PreconditionService | Determinism: same step + config + process → same verdict | unit test (property) |
| 10 | DispatchGate | Refused step's tool is never called (spy asserts); node marked failed | integration test |
| 11 | DispatchGate | Configured-but-unarmed precondition refuses a matching step (`unarmed`) | integration test |
| 12 | PreconditionFinding | Envelope `precondition_findings[]` records outcome + exit code + inputs hash + timestamp; no parameter values or stdout by default | integration test |
| 13 | GatingMode | `release_dependents_on_failure=false` → failed node's dependents marked Skipped, never dispatched | integration test |
| 14 | CompanionStepObligation | `require_companion_step` unmet → deny (or promote) with a finding; present → dispatches | integration test |
| 15 | PreconditionError | All variants, `Display`, `is_retriable()` (all false) | unit test |
| 16 | PreconditionSurfaces | `rigorix_validate_plan` surfaces findings; refusal maps to structured `policy_violation` | integration test |
| 17 | HardeningConfig | `max_failures_before_abort` settable from `rigorix.toml` and changes dispatch | integration test |
| 18 | Attribution | signed `precondition_findings[]` carries `check_digest` / `authority_digest` / `check_writable`; a changed authority changes the signed bytes | unit + envelope test |

---

## Honest Boundary

A precondition **narrows** the window between check and consequence; it does not
close it. There remains a gap between the check at T_check and the action at
T_action. For money-out, the downstream system must also enforce eligibility at
its own commit point. Rigorix's guarantee is narrower and honest: **when the
check says no, the step is refused, and the determination is signed.** This is
the same boundary discipline as ADR-016 (which scopes Phase D out explicitly).

### Boundary strength is mode-dependent

The temporal gap above is not the whole story: the boundary that protects the
check and its authority depends on **how the operator installed it**. Documented
as a mode → guarantee matrix (source of truth: ADR-017 §Honest boundary):

| Mode | Enforced by | Guarantee | Does **not** cover |
|---|---|---|---|
| **default install** (today) | path containment + `PreToolUse` hook | agent-mediated tool calls cannot touch the check/authority | an **unmediated** write path to `$HOME` |
| **`--isolated`** (#985, *available*) | OS ownership/mode (root-owned `0555`/`0444`) | the agent's **UID cannot write** the check/authority | requires no passwordless `sudo` for the agent user |
| **`require_immutable_check`** (#986, *available*) | the engine, at dispatch | a writable check/authority is **refused** | a check writable by a *different* privileged identity |
| **attribution** (#987, *available*) | the signed envelope | a forged/changed authority is **visible** (`authority_digest`) | detection, not prevention |

The **default row is the permanent caveat**: the default install is a policy +
path + hook boundary, not a sandbox. The other rows are opt-in upgrades.
`inputs_hash` hashes the check *inputs*, not the authority; #987's
`authority_digest` binds the authority itself.

`require_immutable_check = true` (per precondition, default `false`) turns the
boundary-strength assessment into a refusal: the runner resolves the check (and
any operator-declared `authority_path`) and assesses writability from POSIX
owner/group/world mode. A writable — or unreadable — boundary fails closed with
`PreconditionError::Boundary`. The default is unchanged: the assessment is
traced, but the run proceeds.

### Attribution — making a forged authority visible

The signed `precondition_findings[]` entry binds the authority, not only the
inputs: `check_digest` (sha256 of the resolved check program), `authority_digest`
(sha256 of the operator-declared `authority_path`, when present), and
`check_writable` (the boundary fact). The engine hashes the files at dispatch —
the check need not cooperate, and raw contents are never promoted into evidence
(SpanPrivacy: digests only). A forged or changed `authority.json` changes
`authority_digest`, so an outsider can detect it against the operator's
expected digest. These fields are additive and omitted when absent; an envelope
without them is pre-attribution, not tampered.

> **Maintenance:** flip a row's state from *planned* to *available* when its
> issue merges; never delete the default caveat.

## Scope Guard (keep the primitive tiny)

The primitive is exactly: *match a step, run a check, refuse on failure, record
the outcome.* Explicitly **not** in scope: loops, chained checks, retries that
change a verdict, a conditionals/policy DSL, multi-step orchestration, or
workflow scheduling. If a use case needs any of those, it is a different module.

## Security Considerations

| Concern | Mitigation |
|---------|------------|
| Agent edits the check to defeat the gate | Config in `.rigorix/**` is denied to agent writes by default (ADR-013 R5; `permission/application/enforcer_impl.rs`). The **command program** must resolve outside the agent-writable workspace; the runner validates and refuses otherwise. This is a path + hook boundary, **not** a sandbox — see §Honest Boundary (mode → guarantee matrix) and ADR-017). |
| Boundary strength is opt-in | `require_immutable_check = true` assesses the resolved check and any declared `authority_path` with POSIX owner/group/world mode (`MetadataExt`/`PermissionsExt` + euid/getgroups). A writable — or unreadable — boundary refuses with `PreconditionError::Boundary` (fail closed). Default `false`: the assessment is traced, behavior unchanged. |
| Command injection | argv only, no shell, no string interpolation of step values into argv. |
| Secret leakage into evidence | Never record parameter values (only `inputs_hash`); never record stdout unless `capture_output=true` (truncated, redacted). |
| Flaky/slow check blocks work | Explicit `timeout_ms`; `error` outcome recorded distinctly from `failed`; operators must keep checks fast/reliable. Fail-closed is deliberate. |
| Silent downgrade | Configured-but-unarmed refuses (never skips). |

## Testing Requirements

| Layer | Test Type | Coverage Target |
|-------|-----------|-----------------|
| Domain | Unit | 95% |
| Application | Unit | 90% |
| Infrastructure | Integration | 85% |

## Dependencies

### Depends On
- **sequence_policy**: `StepPredicate` reuse; `[[requirements]]` extension (R3).
- **execution_engine**: the dispatch choke point (ADR-011).
- **audit**: envelope finding machinery.

### Used By
- **orchestrator**: plan-preview findings.
- **mcp / server**: surfaces + error taxonomy.

## References

- ADR-017 — `.pi/architecture/decisions/ADR-017-consequence-gating.md`
- ADR-011 / ADR-013 / ADR-015 / ADR-016
- `execution_engine/application/service_impl/dispatch.rs`
- `audit/domain/envelope.rs`
