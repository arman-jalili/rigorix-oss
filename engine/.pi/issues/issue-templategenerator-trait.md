---
guardian_issue:
  id: "ISSUE-TEMPLATE-GENERATION-1"
  epic: "TBD"
  component: "TemplateGenerator Trait"
  module: "template-generation"
  status: planned
  priority: high
  dependencies:
    - "none"

  in_scope:
    - Implement TemplateGenerator Trait for the template-generation module
    - Write unit tests for all public interfaces
    - Add integration tests with upstream/downstream components
    - Create API documentation

  out_of_scope:
    - Changes to upstream components (none)
    - UI/frontend changes
    - Deployment pipeline configuration

  affected_layers:
    domain:
      - New domain models for templategenerator-trait
    application:
      - New service/handler for templategenerator-trait
    infrastructure:
      - New database tables or external service connections
    api:
      - New endpoints or event handlers

  canonical_references:
    - module: ".pi/architecture/modules/template-generation.md#templategenerator-trait"

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
    Abstract interface for LLM-based template generation

  file_changes:
    - "create: src/template-generation/templategenerator-trait/"
    - "create: tests/unit/template-generation/templategenerator-trait/"
    - "create: tests/integration/template-generation/templategenerator-trait/"
---

# ISSUE-TEMPLATE-GENERATION-1: TemplateGenerator Trait

## Intent

Abstract interface for LLM-based template generation

## Architecture Context

- **Module:** template-generation
- **Component:** TemplateGenerator Trait
- **Status:** planned
- **Dependencies:** none

## Dependencies

```
  └── none
```

## In Scope

- Implement TemplateGenerator Trait for the template-generation module
- Write unit tests for all public interfaces
- Add integration tests with upstream/downstream components
- Create API documentation

## Out of Scope

- Changes to upstream components
- UI/frontend changes
- Deployment pipeline configuration

## Affected Layers

### Domain
- New domain models for templategenerator-trait

### Application
- New service/handler for templategenerator-trait

### Infrastructure
- New database tables or external service connections

### API
- New endpoints or event handlers

## Canonical References

- **Module:** `.pi/architecture/modules/template-generation.md#templategenerator-trait`

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
