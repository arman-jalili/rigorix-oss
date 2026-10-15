---
guardian_issue:
  id: "ISSUE-CAPACITY-LEDGER-11"
  epic: "TBD"
  component: "CapacityLedger"
  module: "capacity-ledger"
  status: planned
  priority: high
  dependencies:
    - "none"

  in_scope:
    - "Implement CapacityLedger for the capacity-ledger module"
    - "Write unit tests for all public methods"
    - "Add integration tests with upstream/downstream components"

  out_of_scope:
    - Changes to upstream components (none)
    - UI/frontend changes
    - Deployment pipeline configuration

  canonical_references:
    - module: ".pi/architecture/modules/capacity-ledger.md"
    - acceptance_criteria: ".pi/architecture/modules/capacity-ledger.md#acceptance-criteria"

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
    Read .pi/architecture/modules/capacity-ledger.md BEFORE implementing.
    All Acceptance Criteria in that file must be satisfied before this issue is closed.
    Component focus: CapacityLedger.
    CONCRETE IMPLEMENTATIONS MUST BE CREATED — interface stubs from the contract freeze
    are not sufficient. Each domain service/aggregate needs a concrete .impl.rs file.

  file_changes:
    - "create: src/capacity-ledger/domain/"
    - "create: src/capacity-ledger/application/"
    - "create: src/capacity-ledger/infrastructure/"
    - "modify: src/capacity-ledger/interfaces/"
    -     - "update: tests/unit/ (failing tests already generated — make them pass)"
---

# ISSUE-CAPACITY-LEDGER-11: Implement CapacityLedger — capacity-ledger

## Intent

Implement **CapacityLedger** for the `capacity-ledger` module.

> ⚠️ **Read before implementing:** `.pi/architecture/modules/capacity-ledger.md`
> Every item in the **Acceptance Criteria** section of that file must be satisfied
> before this issue is closed — including adapter, mapper, and WireMock items.

## Architecture Context

- **Module:** capacity-ledger
- **Component:** CapacityLedger
- **Status:** planned
- **Dependencies:** none

## In Scope (this component)

- Implement CapacityLedger for the capacity-ledger module
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

- Read .pi/architecture/modules/capacity-ledger.md
- Implement entities and interfaces
- Implement infrastructure (adapter, mapper, repository)
- Implement use case
- Write unit + integration tests
- Run validators
- Create MR

## Implementation

> **Agent instructions:**
> 1. Open `.pi/architecture/modules/capacity-ledger.md` — read the full Acceptance Criteria table
> 2. Identify which rows are your responsibility for **CapacityLedger**
> 3. Create concrete implementation files (`.impl.rs`) in `src/capacity-ledger/` — the interface stubs from the contract freeze are NOT enough
> 4. Each domain aggregate/service must have a working implementation with business logic
> 5. Verify each AC row is satisfied in `src/` before marking done
> 6. Run validators and create MR

### Steps

1. Read canonical architecture references
2. Run the pre-generated failing tests: `cd tests/unit && cargo test`
3. Verify tests FAIL (Red phase)
4. Implement domain entities and interfaces
5. Implement application service/handler
6. Add infrastructure connections
7. Run tests again — they should PASS (Green phase)
8. Refactor if needed (Refactor phase)
9. Write integration tests
10. Run all validators
11. Create MR
