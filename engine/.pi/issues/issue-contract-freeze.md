---
guardian_issue:
  id: "ISSUE-CONTRACT-FREEZE"
  epic: "identity"
  component: "Contract Freeze"
  module: "identity"
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
    - module: ".pi/architecture/modules/identity.md"

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
    - "create: src/identity/domain/"
    - "create: src/identity/application/"
    - "create: src/identity/infrastructure/"
    - "create: src/identity/interfaces/"
---

# Contract Freeze: identity

## Intent

Define and freeze all public interfaces, contracts, and schemas for the identity
epic before any implementation begins. This prevents architecture drift — implementation
must satisfy contracts, not the other way around.

## Included Components

- IdentityClaim
- IdentitySource
- IdentityAttestationService
- TokenVerifier
- IdentityRepository
- IdentityError

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
| 1 | IdentityClaim | Serde round-trip preserves all fields; `redacted_summary()` never contains raw token | unit test |
| 2 | IdentityClaim | `is_valid()` correct across expiry boundary | unit test |
| 3 | IdentityAttestationService | `extract_claims` decodes standard JWT claims (sub, iss, exp, roles) | unit test |
| 4 | IdentityAttestationService | attest with unreachable IdP → `IdentitySource::Unverified`, explicit marker, no error | integration test |
| 5 | IdentityAttestationService | verify against mock JWKS: valid → Verified; tampered → Unverified | integration test |
| 6 | IdentityClaim | RunInput.identity flows into envelope identity block (redacted) | integration test |
| 7 | IdentityAttestationService | ApproveInput.approver_id populated from claim; recorded in ApprovalRecord | integration test |



## Implementation

> **Agent:** Create interface-only files. No implementation. Use Clean Architecture layers:
> 1. Read the architecture module to understand each component's role
> 2. Place domain interfaces in domain/, service interfaces in application/, API contracts in interfaces/http/
> 3. DTOs with proper validation decorators go in application/
> 4. Event schemas go in domain/event/
> 5. Repository interfaces go in infrastructure/repository/
>
> The goal is a reviewed, frozen contract that implementation issues can depend on.
