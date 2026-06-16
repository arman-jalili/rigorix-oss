---
guardian_issue:
  id: "ISSUE-TUI-6"
  epic: "TBD"
  component: "Views"
  module: "tui"
  status: planned
  priority: high
  dependencies:
    - "Renderer"

  in_scope:
    - Implement Views for the tui module
    - Write unit tests for all public interfaces
    - Add integration tests with upstream/downstream components
    - Create API documentation

  out_of_scope:
    - Changes to upstream components (Renderer)
    - UI/frontend changes
    - Deployment pipeline configuration

  affected_layers:
    domain:
      - New domain models for views
    application:
      - New service/handler for views
    infrastructure:
      - New database tables or external service connections
    api:
      - New endpoints or event handlers

  canonical_references:
    - module: ".pi/architecture/modules/tui.md#views"

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
    Full-screen view implementations: Dashboard (DAG tree + details + metrics), Nodes (node list table), Events (filterable timeline), Plan (preview with actions), History (past execution browser), Settings, Templates, Clarification, Diff (plan comparison).

  file_changes:
    - "create: src/tui/views/"
    - "create: tests/unit/tui/views/"
    - "create: tests/integration/tui/views/"
---

# ISSUE-TUI-6: Views

## Intent

Full-screen view implementations: Dashboard (DAG tree + details + metrics), Nodes (node list table), Events (filterable timeline), Plan (preview with actions), History (past execution browser), Settings, Templates, Clarification, Diff (plan comparison).

## Architecture Context

- **Module:** tui
- **Component:** Views
- **Status:** planned
- **Dependencies:** Renderer

## Dependencies

```
  └── Renderer
```

## In Scope

- Implement Views for the tui module
- Write unit tests for all public interfaces
- Add integration tests with upstream/downstream components
- Create API documentation

## Out of Scope

- Changes to upstream components
- UI/frontend changes
- Deployment pipeline configuration

## Affected Layers

### Domain
- New domain models for views

### Application
- New service/handler for views

### Infrastructure
- New database tables or external service connections

### API
- New endpoints or event handlers

## Canonical References

- **Module:** `.pi/architecture/modules/tui.md#views`

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
