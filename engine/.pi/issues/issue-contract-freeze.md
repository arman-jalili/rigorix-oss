---
guardian_issue:
  id: "ISSUE-CONTRACT-FREEZE"
  epic: ""error-handling""
  component: "Contract Freeze"
<<<<<<< HEAD
  module: "audit"
=======
  module: "error-handling"
>>>>>>> origin/main
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
<<<<<<< HEAD
    - module: ".pi/architecture/modules/audit.md"
=======
    - module: ".pi/architecture/modules/error-handling.md"
>>>>>>> origin/main

  acceptance_criteria:
    - "All component interfaces defined as interfaces/types"
    - "DTO schemas documented"
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
<<<<<<< HEAD
    - "create: src/audit/contracts/"
    - "create: src/audit/contracts/dtos/"
    - "create: src/audit/contracts/events/"
---

# Contract Freeze: audit

## Intent

Define and freeze all public interfaces, contracts, and schemas for the audit
=======
    - "create: src/error-handling/contracts/"
    - "create: src/error-handling/contracts/dtos/"
    - "create: src/error-handling/contracts/events/"
---

# Contract Freeze: error-handling

## Intent

Define and freeze all public interfaces, contracts, and schemas for the error-handling
>>>>>>> origin/main
epic before any implementation begins. This prevents architecture drift — implementation
must satisfy contracts, not the other way around.

## Included Components

<<<<<<< HEAD
- AuditEnvelope
- AuditSender
- AuditQueue
- CircuitBreaker
=======
- CoreOrchestratorError
>>>>>>> origin/main

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
| 1 | All component interfaces defined | Check src/<group>/<module>/domain/ and application/ |
| 2 | Contracts reviewed and frozen | PR approval |
| 3 | DTO schemas documented | OpenAPI / TypeSpec / equivalent |
| 4 | Implementation depends on contracts | No implementation without interface |

## Implementation

> **Agent:** Create interface-only files. No implementation. Use Clean Architecture layers:
> 1. Read the architecture module to understand each component's role
> 2. Place domain interfaces in domain/, service interfaces in application/, API contracts in interfaces/http/
> 3. DTOs with proper validation decorators go in application/
> 4. Event schemas go in domain/event/
> 5. Repository interfaces go in infrastructure/repository/
>
> The goal is a reviewed, frozen contract that implementation issues can depend on.
