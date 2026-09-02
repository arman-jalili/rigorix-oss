---
guardian_issue:
  id: "ISSUE-CONTRACT-FREEZE"
  epic: "approval"
  component: "Contract Freeze"
  module: "approval"
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
    - module: ".pi/architecture/modules/approval.md"

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
    - "create: src/approval/domain/"
    - "create: src/approval/application/"
    - "create: src/approval/infrastructure/"
    - "create: src/approval/interfaces/"
---

# Contract Freeze: approval

## Intent

Define and freeze all public interfaces, contracts, and schemas for the approval
epic before any implementation begins. This prevents architecture drift — implementation
must satisfy contracts, not the other way around.

## Included Components

- ExecutionIntent
- IntentHash
- ApprovalRecord
- DecisionContext
- ScopeViolation
- ApprovalService
- ApproveInput / ApproveOutput
- ApprovalError

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
| 1 | ExecutionIntent | `from_node` on sealed node produces canonical intent; `canonical_bytes` deterministic (property test) | unit test |
| 2 | IntentHash | Same tool+intent+scope → same hash; any byte change → different hash | unit test |
| 3 | ApprovalRecord | Serde round-trip preserves all fields; status transitions valid (Pending → Consumed/Expired/Superseded) | unit test |
| 4 | ApprovalService | approve → dispatch identical intent → executes; envelope contains signed `ApprovalRecorded` | integration test |
| 5 | ApprovalService | approve → intent mutated (simulated upstream change / tampered state) → **HALT before dispatch**, `IntentMismatch`, tool never called (spy tool asserts) | integration test |
| 6 | ApprovalService | Cross-process: pause in A, tamper persisted intent in state file, resume in B → halted, re-approval required | integration test |
| 7 | ApprovalService | Retry path: failed approved node retries with same intent → per-attempt verify passes; a failed attempt does NOT consume; consume happens once on terminal outcome | integration test |
| 8 | ApprovalService | Replay: consumed approval replayed against same step → rejected (single-use + nonce) | integration test |
| 9 | ApprovalService | TTL: approval expires between approve and dispatch → `Invalid(Expired)`, no dispatch | unit test |
| 10 | ApprovalService | Migration: persisted `approved` without records → invalidated on hydrate, re-approval required | integration test |
| 11 | ScopeViolation | File tool outside declared scope → `scope_violation` flagged in envelope | integration test |
| 12 | ScopeViolation | `run_command` side-effect on `src/auth.ts` (script) → caught by **git-diff oracle** → `scope_violation` | integration test |
| 13 | DecisionContext | approve with decision_context → envelope contains context ref + summary; full payload opt-in | integration test |
| 14 | DecisionContext | api_key inside decision_context → redacted in summary (SpanPrivacy reuse) | unit test |
| 15 | ApprovalError | All variants, `Display`, `is_retriable()` (IntentMismatch → non-retriable) | unit test |



## Implementation

> **Agent:** Create interface-only files. No implementation. Use Clean Architecture layers:
> 1. Read the architecture module to understand each component's role
> 2. Place domain interfaces in domain/, service interfaces in application/, API contracts in interfaces/http/
> 3. DTOs with proper validation decorators go in application/
> 4. Event schemas go in domain/event/
> 5. Repository interfaces go in infrastructure/repository/
>
> The goal is a reviewed, frozen contract that implementation issues can depend on.
