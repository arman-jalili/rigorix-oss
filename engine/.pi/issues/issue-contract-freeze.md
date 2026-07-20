---
guardian_issue:
  id: "ISSUE-CONTRACT-FREEZE"
  epic: "scored-evaluation"
  component: "Contract Freeze"
  module: "scored-evaluation"
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
    - module: ".pi/architecture/modules/scored-evaluation.md"

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
    - "create: src/scored-evaluation/domain/"
    - "create: src/scored-evaluation/application/"
    - "create: src/scored-evaluation/infrastructure/"
    - "create: src/scored-evaluation/interfaces/"
---

# Contract Freeze: scored-evaluation

## Intent

Define and freeze all public interfaces, contracts, and schemas for the scored-evaluation
epic before any implementation begins. This prevents architecture drift — implementation
must satisfy contracts, not the other way around.

## Included Components

- Domain Layer (`domain/`)
- Application Layer (`application/`)
- Infrastructure Layer (`infrastructure/`)
- ScoredEvaluationNode
- Rubric
- ScoringResult
- ScoringBackend (Trait)
- ScoredEvaluationEvent
- ScoredEvaluationError
- ScoredEvaluationService
- EvaluateInput / EvaluateOutput
- MCPBackend
- HTTPBackend
- LocalBackend
- ScoreDimension
- ScoringBackend
- ScoreAbove policy condition
- ScoreBelow policy condition

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
| 1 | ScoredEvaluationNode | Serde round-trip: serialize and deserialize preserves all fields | unit test |
| 2 | Rubric | Inline and Reference sources serialize correctly with tagged enum | unit test |
| 3 | ScoringResult | `passed` flag computed correctly from per-dimension `passed` | unit test |
| 4 | ScoreDimension | `passed` correctly calculated from score vs max | unit test |
| 5 | ScoringBackend | All three backend implementations pass same trait contract test suite | integration test |
| 6 | MCPBackend | Sends `rigorix_evaluate_artifact` and parses MCP response into ScoringResult | integration test |
| 7 | HTTPBackend | POSTs artifact + rubric to URL, parses JSON response | integration test |
| 8 | LocalBackend | Executes script with env vars, reads scoring result from stdout | integration test |
| 9 | ScoredEvaluationService | Orchestrates: validate → emit Started → backend → emit Completed/Failed → persist | integration test |
| 10 | ScoredEvaluationEvent | All three event variants serialize/deserialize correctly | unit test |
| 11 | ScoredEvaluationError | All 7 variants, `Display` impl, `is_retriable()` classification correct | unit test |
| 12 | ScoreAbove policy condition | Policy condition correctly gates merge when dimension(s) below threshold | integration test |
| 13 | ScoreBelow policy condition | Policy condition correctly blocks merge when any dimension below threshold | integration test |



## Implementation

> **Agent:** Create interface-only files. No implementation. Use Clean Architecture layers:
> 1. Read the architecture module to understand each component's role
> 2. Place domain interfaces in domain/, service interfaces in application/, API contracts in interfaces/http/
> 3. DTOs with proper validation decorators go in application/
> 4. Event schemas go in domain/event/
> 5. Repository interfaces go in infrastructure/repository/
>
> The goal is a reviewed, frozen contract that implementation issues can depend on.
