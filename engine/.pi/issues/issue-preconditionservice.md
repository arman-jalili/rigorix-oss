---
guardian_issue:
  id: "ISSUE-CONSEQUENCE-GATING-4"
  epic: "TBD"
  component: "PreconditionService"
  module: "consequence-gating"
  status: planned
  priority: high
  dependencies:
    - "Precondition"
    - "PreconditionRunner"

  in_scope:
    - "Matched step exit 0 → `Dispatch`; non-zero → `Deny`"
    - "`require_params` absent → `Deny` before the command runs"
    - "Non-matching step is unaffected (no process spawned)"
    - "Determinism: same step + config + process → same verdict"

  out_of_scope:
    - Changes to upstream components (Precondition, PreconditionRunner)
    - UI/frontend changes
    - Deployment pipeline configuration

  canonical_references:
    - module: ".pi/architecture/modules/consequence-gating.md"
    - acceptance_criteria: ".pi/architecture/modules/consequence-gating.md#acceptance-criteria"

  acceptance_criteria:
    - "Matched step exit 0 → `Dispatch`; non-zero → `Deny`"
    - "`require_params` absent → `Deny` before the command runs"
    - "Non-matching step is unaffected (no process spawned)"
    - "Determinism: same step + config + process → same verdict"

  validators:
    - ci
    - tests
    - security
    - architecture
    - canonical

  implementation_notes: |
    Read .pi/architecture/modules/consequence-gating.md BEFORE implementing.
    All Acceptance Criteria in that file must be satisfied before this issue is closed.
    Component focus: PreconditionService.
    CONCRETE IMPLEMENTATIONS MUST BE CREATED — interface stubs from the contract freeze
    are not sufficient. Each domain service/aggregate needs a concrete .impl.rs file.

  file_changes:
    - "create: src/consequence-gating/domain/"
    - "create: src/consequence-gating/application/"
    - "create: src/consequence-gating/infrastructure/"
    - "modify: src/consequence-gating/interfaces/"
    -     - "update: tests/unit/ (failing tests already generated — make them pass)"
---

# ISSUE-CONSEQUENCE-GATING-4: Implement PreconditionService — consequence-gating

## Intent

Implement **PreconditionService** for the `consequence-gating` module.

> ⚠️ **Read before implementing:** `.pi/architecture/modules/consequence-gating.md`
> Every item in the **Acceptance Criteria** section of that file must be satisfied
> before this issue is closed — including adapter, mapper, and WireMock items.

## Architecture Context

- **Module:** consequence-gating
- **Component:** PreconditionService
- **Status:** planned
- **Dependencies:** Precondition, PreconditionRunner

## In Scope (this component)

- Matched step exit 0 → `Dispatch`; non-zero → `Deny`
- `require_params` absent → `Deny` before the command runs
- Non-matching step is unaffected (no process spawned)
- Determinism: same step + config + process → same verdict

## Acceptance Criteria (this issue)

These acceptance criteria must be satisfied before this issue can be closed:

| # | Criterion | Verify In |
|---|-----------|-----------|
| 6 | Matched step exit 0 → `Dispatch`; non-zero → `Deny` | unit test |
| 7 | `require_params` absent → `Deny` before the command runs | unit test |
| 8 | Non-matching step is unaffected (no process spawned) | unit test |
| 9 | Determinism: same step + config + process → same verdict | unit test (property) |

## Full Module Acceptance Criteria

> All items below must pass before the **epic** is closed.
> Items may be split across multiple issues — verify your component's items before creating the MR.

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

## Implementation Sequence (from module doc)

- 1. Contract freeze (rigorix-sdk #39): `rigorix-sdk/schemas/policy.json` (preconditions + `[gating]`) + `schemas/envelope.json` (`precondition_findings[]`) + `schemas/api/errors.json` + a signed fixture verified byte-exact in Rust/Python/TypeScript/Java/Go.
- 2. Domain: `engine/src/precondition/domain/{precondition,gating,error}.rs` + safety caps and fail-closed config validation.
- 3. Infrastructure/config: `engine/src/precondition/infrastructure/toml_repository.rs` for `.rigorix/preconditions.toml` (missing ⇒ `Ok(None)`, malformed/over-cap ⇒ `Err`).
- 4. Infrastructure/runner: `engine/src/precondition/infrastructure/runner.rs` — argv only, JSON stdin, env, wall-clock timeout, and the trust-boundary check that `command[0]` resolves outside the agent-writable workspace.
- 5. Application: `engine/src/precondition/application/{service,service_impl}.rs` — match step, enforce `require_params` presence, run the check, return Dispatch/Deny.
- 6. Dispatch gate: `engine/src/execution_engine/application/service_impl/dispatch.rs` (after ADR-011 `verify_before_dispatch`, before `spawn_concurrent_node`), plus `engine/src/execution_engine/application/factory.rs` and the composition roots `cli/src/cli_boundary/orchestrator.rs`, `actions/src/main.rs`, `server/src/**`.
- 7. Evidence: `engine/src/event_system/domain/event.rs` (`PreconditionChecked`) + `engine/src/audit/domain/envelope.rs` + `engine/src/audit/application/envelope_factory_impl.rs` (SpanPrivacy: no values, no stdout by default).
- 8. R2 step-outcome gating: `engine/src/precondition/domain/gating.rs` + the release-dependents logic in `engine/src/execution_engine/application/service_impl/dispatch.rs`.
- 9. R3 companion step: `engine/src/sequence_policy/domain/{requirement,config}.rs` + `engine/src/sequence_policy/application/service_impl.rs` (only when this phase is in scope).
- 10. Surfaces: `mcp/src/execution_tools/**` + `mcp/src/host/error.rs` + `server/src/version.rs` (no new `rigorix.*` method).
- 11. Hardening: `engine/src/configuration/domain/config.rs` (`max_failures_before_abort` reachable) + fail-closed arming in `engine/src/execution_engine/application/factory.rs`.
- 12. Falsifier/demo: `engine/tests/precondition_e2e.rs` + `demo/consequence-gating/run.sh` (T0 approve → ΔN flip → Tn refuse + signed evidence).

## Implementation

> **Agent instructions:**
> 1. Open `.pi/architecture/modules/consequence-gating.md` — read the full Acceptance Criteria table
> 2. Identify which rows are your responsibility for **PreconditionService**
> 3. Create concrete implementation files (`.impl.rs`) in `src/consequence-gating/` — the interface stubs from the contract freeze are NOT enough
> 4. Each domain aggregate/service must have a working implementation with business logic
> 5. Verify each AC row is satisfied in `src/` before marking done
> 6. Run validators and create MR

### Steps

1. Read canonical architecture references
2. Run the pre-generated failing tests: `cd tests/unit && cargo test`
3. Verify tests FAIL (Red phase)
4. Implement domain entities and interfaces
5. Implement application service/handler
6. Add infrastructure connections
7. Run tests again — they should PASS (Green phase)
8. Refactor if needed (Refactor phase)
9. Write integration tests
10. Run all validators
11. Create MR
