---
guardian_issue:
  id: "ISSUE-EXECUTION-TOOLS-3"
  epic: "TBD"
  component: "ValidatePlanHandler (Domain Service)"
  module: "execution-tools"
  status: planned
  priority: high
  dependencies:
    - "none"

  in_scope:
    - Implement ValidatePlanHandler (Domain Service) for the execution-tools module
    - Write unit tests for all public interfaces
    - Add integration tests with upstream/downstream components
    - Create API documentation

  out_of_scope:
    - Changes to upstream components (none)
    - UI/frontend changes
    - Deployment pipeline configuration

  affected_layers:
    domain:
      - New domain models for validateplanhandler-(domain-service)
    application:
      - New service/handler for validateplanhandler-(domain-service)
    infrastructure:
      - New database tables or external service connections
    api:
      - New endpoints or event handlers

  canonical_references:
    - module: ".pi/architecture/modules/execution-tools.md#validateplanhandler-(domain-service)"

  acceptance_criteria:
    - "CI pipeline passes (validate-ci.sh)"
    - "All unit tests pass with ≥ 90% coverage"
    - "Integration tests pass with upstream/downstream components"
    - "validate-security.sh passes"
    - "validate-architecture.sh passes"
    - "validate-canonical.sh passes"

  validators:
    - ci
    - tests
    - security
    - architecture
    - canonical

  implementation_notes: |
    Handles `rigorix_validate_plan` tool calls: checks plan against enforcement policies.

  file_changes:
    - "modify: src/execution-tools/domain/"
    - "modify: src/execution-tools/application/"
    - "modify: src/execution-tools/infrastructure/"
    - "modify: src/execution-tools/interfaces/"
    - "create: tests/unit/execution-tools/validateplanhandler-(domain-service)/"
    - "create: tests/integration/execution-tools/validateplanhandler-(domain-service)/"
---

# ISSUE-EXECUTION-TOOLS-3: ValidatePlanHandler (Domain Service)

## Intent

Handles `rigorix_validate_plan` tool calls: checks plan against enforcement policies.

## Architecture Context

- **Module:** execution-tools
- **Component:** ValidatePlanHandler (Domain Service)
- **Status:** planned
- **Dependencies:** none

## Dependencies

```
  └── none
```

## In Scope

- Implement ValidatePlanHandler (Domain Service) for the execution-tools module
- Write unit tests for all public interfaces
- Add integration tests with upstream/downstream components
- Create API documentation

## Out of Scope

- Changes to upstream components
- UI/frontend changes
- Deployment pipeline configuration

## Affected Layers

### Domain
- New domain models for validateplanhandler-(domain-service)

### Application
- New service/handler for validateplanhandler-(domain-service)

### Infrastructure
- New database tables or external service connections

### API
- New endpoints or event handlers

## Canonical References

- **Module:** `.pi/architecture/modules/execution-tools.md#validateplanhandler-(domain-service)`

## Acceptance Criteria

| # | Criterion | Validator |
|---|-----------|-----------|
| 1 | CI pipeline passes | `validate-ci.sh` |
| 2 | All unit tests pass with ≥ 90% coverage | `validate-tests.sh` |
| 3 | Integration tests pass | `validate-integration.sh` |
| 4 | Security checks pass | `validate-security.sh` |
| 5 | Architecture compliance | `validate-architecture.sh` |
| 6 | Canonical references valid | `validate-canonical.sh` |

## Implementation

> **Agent:** This is your complete session context. All information you need is above.
> Start by reading the canonical reference files, then implement following the layer structure.

### Steps

1. Read canonical architecture references
2. Create domain entities and interfaces
3. Implement application service/handler
4. Add infrastructure connections
5. Write unit tests (≥ 90% coverage)
6. Write integration tests
7. Run all validators
8. Create MR
