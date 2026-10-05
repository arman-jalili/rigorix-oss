---
guardian_issue:
  id: "ISSUE-CONTRACT-FREEZE"
  epic: "consequence-gating"
  component: "Contract Freeze"
  module: "consequence-gating"
  status: planned
  priority: critical
  dependencies: []

  in_scope:
    - Define public interfaces for all components in this epic
    - Define DTOs, schemas, and API contracts
    - Document event payloads and topics
    - Create interface stubs with no implementation
    - Freeze: no implementation changes without contract change

  out_of_scope:
    - Any implementation logic
    - Database schema changes
    - Infrastructure setup

  affected_layers:
    domain:
      - Interface definitions for domain services
    application:
      - Input/output DTO definitions
    api:
      - REST/event contracts

  canonical_references:
    - module: ".pi/architecture/modules/precondition.md"

  acceptance_criteria:
    - "All component interfaces defined as stubs (TODO bodies)"
    - "DTO schemas documented with field names and types"
    - "API contracts frozen and reviewed"
    - "Implementation PRs reference these contracts"

  validators:
    - architecture
    - canonical

  implementation_notes: |
    Define the contract before any implementation. Every implementation issue
    depends on this contract being frozen first. The contract should include:
    interfaces, types, DTOs, event schemas, API paths, error formats.

  file_changes:
    - "create: src/consequence-gating/domain/"
    - "create: src/consequence-gating/application/"
    - "create: src/consequence-gating/infrastructure/"
    - "create: src/consequence-gating/interfaces/"
---

# Contract Freeze: consequence-gating

## Intent

Define and freeze all public interfaces, contracts, and schemas for the consequence-gating
epic before any implementation begins. This prevents architecture drift — implementation
must satisfy contracts, not the other way around.

## Included Components

- Precondition
- PreconditionRepository
- PreconditionRunner
- PreconditionService
- DispatchGate
- PreconditionFinding
- GatingMode
- CompanionStepObligation
- PreconditionError
- PreconditionSurfaces
- HardeningConfig

## What Must Be Frozen

### Interfaces
- Service interfaces for every component
- Repository/DAO interfaces
- Factory interfaces

### Contracts
- Input/output DTO schemas
- API endpoint contracts (method, path, request/response)
- Event payload schemas
- Error response formats

### Out of Bounds (no contracts needed)
- Internal implementation details
- Database column names (hidden behind repository)
- Framework-specific annotations

## Acceptance Criteria

| # | Criterion | How to Verify |
|---|-----------|---------------|
| 1 | All component interfaces defined as stubs (TODO bodies) | Check src/<module>/domain/ and application/ |
| 2 | Contracts reviewed and frozen | PR approval |
| 3 | DTO schemas documented with field names and types | OpenAPI / record types |
| 4 | Implementation depends on contracts | No implementation without interface |

## Full Module Acceptance Criteria (for reference)

> These are the complete ACs for the module. The contract freeze must define the interfaces
> so every row below can be implemented in subsequent issues.

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

## Full Implementation Sequence (for reference)

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

> **Agent:** Create interface-only files. No implementation. Use Clean Architecture layers:
> 1. Read the architecture module to understand each component's role
> 2. Place domain interfaces in domain/, service interfaces in application/, API contracts in interfaces/http/
> 3. DTOs with proper validation decorators go in application/
> 4. Event schemas go in domain/event/
> 5. Repository interfaces go in infrastructure/repository/
>
> The goal is a reviewed, frozen contract that implementation issues can depend on.
