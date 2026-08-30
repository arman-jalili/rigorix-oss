---
guardian_issue:
  id: "ISSUE-IDENTITY-4"
  epic: "TBD"
  component: "TokenVerifier"
  module: "identity"
  status: planned
  priority: high
  dependencies:
    - "none"

  in_scope:
    - "Serde round-trip preserves all fields; `redacted_summary()` never contains raw token"
    - "`is_valid()` correct across expiry boundary"

  out_of_scope:
    - Changes to upstream components (none)
    - UI/frontend changes
    - Deployment pipeline configuration

  canonical_references:
    - module: ".pi/architecture/modules/identity.md"
    - acceptance_criteria: ".pi/architecture/modules/identity.md#acceptance-criteria"

  acceptance_criteria:
    - "Serde round-trip preserves all fields; `redacted_summary()` never contains raw token"
    - "`is_valid()` correct across expiry boundary"

  validators:
    - ci
    - tests
    - security
    - architecture
    - canonical

  implementation_notes: |
    Read .pi/architecture/modules/identity.md BEFORE implementing.
    All Acceptance Criteria in that file must be satisfied before this issue is closed.
    Component focus: TokenVerifier.
    CONCRETE IMPLEMENTATIONS MUST BE CREATED — interface stubs from the contract freeze
    are not sufficient. Each domain service/aggregate needs a concrete .impl.rs file.

  file_changes:
    - "create: src/identity/domain/"
    - "create: src/identity/application/"
    - "create: src/identity/infrastructure/"
    - "modify: src/identity/interfaces/"
    -     - "update: tests/unit/ (failing tests already generated — make them pass)"
---

# ISSUE-IDENTITY-4: Implement TokenVerifier — identity

## Intent

Implement **TokenVerifier** for the `identity` module.

> ⚠️ **Read before implementing:** `.pi/architecture/modules/identity.md`
> Every item in the **Acceptance Criteria** section of that file must be satisfied
> before this issue is closed — including adapter, mapper, and WireMock items.

## Architecture Context

- **Module:** identity
- **Component:** TokenVerifier
- **Status:** planned
- **Dependencies:** none

## In Scope (this component)

- Serde round-trip preserves all fields; `redacted_summary()` never contains raw token
- `is_valid()` correct across expiry boundary

## Acceptance Criteria (this issue)

These acceptance criteria must be satisfied before this issue can be closed:

| # | Criterion | Verify In |
|---|-----------|-----------|
| 1 | Serde round-trip preserves all fields; `redacted_summary()` never contains raw token | unit test |
| 2 | `is_valid()` correct across expiry boundary | unit test |

## Full Module Acceptance Criteria

> All items below must pass before the **epic** is closed.
> Items may be split across multiple issues — verify your component's items before creating the MR.

| # | Component | Criterion | Verify In |
|---|-----------|-----------|-----------|
| 1 | IdentityClaim | Serde round-trip preserves all fields; `redacted_summary()` never contains raw token | unit test |
| 2 | IdentityClaim | `is_valid()` correct across expiry boundary | unit test |
| 3 | IdentityAttestationService | `extract_claims` decodes standard JWT claims (sub, iss, exp, roles) | unit test |
| 4 | IdentityAttestationService | attest with unreachable IdP → `IdentitySource::Unverified`, explicit marker, no error | integration test |
| 5 | IdentityAttestationService | verify against mock JWKS: valid → Verified; tampered → Unverified | integration test |
| 6 | IdentityClaim | RunInput.identity flows into envelope identity block (redacted) | integration test |
| 7 | IdentityAttestationService | ApproveInput.approver_id populated from claim; recorded in ApprovalRecord | integration test |

## Implementation Sequence (from module doc)

- Read .pi/architecture/modules/identity.md
- Implement entities and interfaces
- Implement infrastructure (adapter, mapper, repository)
- Implement use case
- Write unit + integration tests
- Run validators
- Create MR

## Implementation

> **Agent instructions:**
> 1. Open `.pi/architecture/modules/identity.md` — read the full Acceptance Criteria table
> 2. Identify which rows are your responsibility for **TokenVerifier**
> 3. Create concrete implementation files (`.impl.rs`) in `src/identity/` — the interface stubs from the contract freeze are NOT enough
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
