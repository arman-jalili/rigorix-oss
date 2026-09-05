---
guardian_issue:
  id: "ISSUE-CONTRACT-FREEZE"
  epic: "sequence-policy"
  component: "Contract Freeze"
  module: "sequence-policy"
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
    - module: ".pi/architecture/modules/sequence-policy.md"

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
    - "create: src/sequence-policy/domain/"
    - "create: src/sequence-policy/application/"
    - "create: src/sequence-policy/infrastructure/"
    - "create: src/sequence-policy/interfaces/"
---

# Contract Freeze: sequence-policy

## Intent

Define and freeze all public interfaces, contracts, and schemas for the sequence-policy
epic before any implementation begins. This prevents architecture drift — implementation
must satisfy contracts, not the other way around.

## Included Components

- SequenceRule
- SequencePolicyService
- SequencePolicyError
- StepPredicate
- Matcher
- Orchestrator (R2)
- MCP surface
- Execution Engine (R3)
- Fail-closed
- Fail-open-absent
- Audit (R6)
- Permission (R5)

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
| 1 | SequenceRule | TOML parse → rule with ordered predicates + action; serde round-trip preserves all fields | unit test |
| 2 | StepPredicate | Tool glob + param exact/glob/regex match correctly; non-matching params do not match | unit test |
| 3 | Matcher | Adjacent pair match; windowed match (gap ≤ window); out-of-window does not match | unit test |
| 4 | Matcher | Determinism property test: same ordered plan + rules → same match set | unit test (property) |
| 5 | SequencePolicyService | `evaluate_plan` finds remove-then-add pair in a runbook; returns later step id | unit test |
| 6 | Orchestrator (R2) | Runbook containing remove-then-add: later step is built `requires_approval=true`; run pauses; approve → executes; reject → skipped | integration test |
| 7 | Orchestrator (R2) | `deny` rule: later step fails with `SequencePolicyDenied`, tool never called (spy tool asserts) | integration test |
| 8 | MCP surface | `rigorix_validate_plan` on a plan with a matched sequence returns a structured finding before run | integration test |
| 9 | Execution Engine (R3) | Dynamic plan completes step A, then proposes B (would complete) → B promoted/denied per rule | integration test |
| 10 | Fail-closed | Corrupt rule config → plan refused, no steps execute | integration test |
| 11 | Fail-open-absent | No config file → run executes unchanged | integration test |
| 12 | Audit (R6) | Matched rule + promotion recorded in envelope events; decision summaries redact parameter values by default (SpanPrivacy pattern) | integration test |
| 13 | Permission (R5) | `workspace_write` agent file-write to `.rigorix/**` denied by default permission config | integration test |
| 14 | SequencePolicyError | All variants, `Display`, `is_retriable()` | unit test |



## Implementation

> **Agent:** Create interface-only files. No implementation. Use Clean Architecture layers:
> 1. Read the architecture module to understand each component's role
> 2. Place domain interfaces in domain/, service interfaces in application/, API contracts in interfaces/http/
> 3. DTOs with proper validation decorators go in application/
> 4. Event schemas go in domain/event/
> 5. Repository interfaces go in infrastructure/repository/
>
> The goal is a reviewed, frozen contract that implementation issues can depend on.
