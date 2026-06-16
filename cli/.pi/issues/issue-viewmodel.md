---
guardian_issue:
  id: "ISSUE-TUI-4"
  epic: "TBD"
  component: "ViewModel"
  module: "tui"
  status: planned
  priority: high
  dependencies:
    - "EventBridge"

  in_scope:
    - Implement ViewModel for the tui module
    - Write unit tests for all public interfaces
    - Add integration tests with upstream/downstream components
    - Create API documentation

  out_of_scope:
    - Changes to upstream components (EventBridge)
    - UI/frontend changes
    - Deployment pipeline configuration

  affected_layers:
    domain:
      - New domain models for viewmodel
    application:
      - New service/handler for viewmodel
    infrastructure:
      - New database tables or external service connections
    api:
      - New endpoints or event handlers

  canonical_references:
    - module: ".pi/architecture/modules/tui.md#viewmodel"

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
    Root state model with double-buffering — `TuiViewModel` (execution phase, DAG tree, event log, metrics, budget), `DagViewModel` (node tree), `NodeViewModel`, `MetricsViewModel`, `LlmBudgetViewModel`. Write buffer written by EventBridge, read buffer consumed by render loop.

  file_changes:
    - "create: src/tui/viewmodel/"
    - "create: tests/unit/tui/viewmodel/"
    - "create: tests/integration/tui/viewmodel/"
---

# ISSUE-TUI-4: ViewModel

## Intent

Root state model with double-buffering — `TuiViewModel` (execution phase, DAG tree, event log, metrics, budget), `DagViewModel` (node tree), `NodeViewModel`, `MetricsViewModel`, `LlmBudgetViewModel`. Write buffer written by EventBridge, read buffer consumed by render loop.

## Architecture Context

- **Module:** tui
- **Component:** ViewModel
- **Status:** planned
- **Dependencies:** EventBridge

## Dependencies

```
  └── EventBridge
```

## In Scope

- Implement ViewModel for the tui module
- Write unit tests for all public interfaces
- Add integration tests with upstream/downstream components
- Create API documentation

## Out of Scope

- Changes to upstream components
- UI/frontend changes
- Deployment pipeline configuration

## Affected Layers

### Domain
- New domain models for viewmodel

### Application
- New service/handler for viewmodel

### Infrastructure
- New database tables or external service connections

### API
- New endpoints or event handlers

## Canonical References

- **Module:** `.pi/architecture/modules/tui.md#viewmodel`

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
