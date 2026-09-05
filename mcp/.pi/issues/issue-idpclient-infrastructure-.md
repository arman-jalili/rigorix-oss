---
guardian_issue:
  id: "ISSUE-AUTH-2"
  epic: "TBD"
  component: "IdpClient (Infrastructure)"
  module: "auth"
  status: planned
  priority: high
  dependencies:
    - "none"

  in_scope:
    - "Implement IdpClient (Infrastructure) for the auth module"
    - "Write unit tests for all public methods"
    - "Add integration tests with upstream/downstream components"

  out_of_scope:
    - Changes to upstream components (none)
    - UI/frontend changes
    - Deployment pipeline configuration

  canonical_references:
    - module: ".pi/architecture/modules/auth.md"
    - acceptance_criteria: ".pi/architecture/modules/auth.md#acceptance-criteria"

  acceptance_criteria:
    - "CI pipeline passes (validate-ci.sh)"
    - "All unit tests pass"
    - "Architecture compliance (validate-architecture.sh)"
    - "Canonical references valid (validate-canonical.sh)"

  validators:
    - ci
    - tests
    - security
    - architecture
    - canonical

  implementation_notes: |
    Read .pi/architecture/modules/auth.md BEFORE implementing.
    All Acceptance Criteria in that file must be satisfied before this issue is closed.
    Component focus: IdpClient (Infrastructure).
    CONCRETE IMPLEMENTATIONS MUST BE CREATED — interface stubs from the contract freeze
    are not sufficient. Each domain service/aggregate needs a concrete .impl.ts file.

  file_changes:
    - "create: src/auth/domain/"
    - "create: src/auth/application/"
    - "create: src/auth/infrastructure/"
    - "modify: src/auth/interfaces/"
    -     - "create: tests/unit/"
---

# ISSUE-AUTH-2: Implement IdpClient (Infrastructure) — auth

## Intent

Implement **IdpClient (Infrastructure)** for the `auth` module.

> ⚠️ **Read before implementing:** `.pi/architecture/modules/auth.md`
> Every item in the **Acceptance Criteria** section of that file must be satisfied
> before this issue is closed — including adapter, mapper, and WireMock items.

## Architecture Context

- **Module:** auth
- **Component:** IdpClient (Infrastructure)
- **Status:** planned
- **Dependencies:** none

## In Scope (this component)

- Implement IdpClient (Infrastructure) for the auth module
- Write unit tests for all public methods
- Add integration tests with upstream/downstream components

## Acceptance Criteria (this issue)

These acceptance criteria must be satisfied before this issue can be closed:

| # | Criterion | Verify In |
|---|-----------|-----------|
| 1 | CI pipeline passes | `validate-ci.sh` |
| 2 | All unit tests pass | `validate-tests.sh` |
| 3 | Architecture compliance | `validate-architecture.sh` |
| 4 | Canonical references valid | `validate-canonical.sh` |

## Full Module Acceptance Criteria

> All items below must pass before the **epic** is closed.
> Items may be split across multiple issues — verify your component's items before creating the MR.

| # | Criterion | Validator |
|---|-----------|-----------|
| 1 | CI pipeline passes | `validate-ci.sh` |
| 2 | All unit tests pass | `validate-tests.sh` |
| 3 | Integration tests pass | `validate-integration.sh` |
| 4 | Architecture compliance | `validate-architecture.sh` |
| 5 | Canonical references valid | `validate-canonical.sh` |

## Implementation Sequence (from module doc)

- Read .pi/architecture/modules/auth.md
- Implement entities and interfaces
- Implement infrastructure (adapter, mapper, repository)
- Implement use case
- Write unit + integration tests
- Run validators
- Create MR

## Implementation

> **Agent instructions:**
> 1. Open `.pi/architecture/modules/auth.md` — read the full Acceptance Criteria table
> 2. Identify which rows are your responsibility for **IdpClient (Infrastructure)**
> 3. Create concrete implementation files (`.impl.ts`) in `src/auth/` — the interface stubs from the contract freeze are NOT enough
> 4. Each domain aggregate/service must have a working implementation with business logic
> 5. Verify each AC row is satisfied in `src/` before marking done
> 6. Run validators and create MR

### Steps

1. Read canonical architecture references
2. Create domain entities and interfaces
3. Implement application service/handler
4. Add infrastructure connections
5. Write unit tests (≥ 90% coverage)
6. Write integration tests
7. Run all validators
8. Create MR
