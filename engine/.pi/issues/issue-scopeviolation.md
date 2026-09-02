---
guardian_issue:
  id: "ISSUE-APPROVAL-5"
  epic: "TBD"
  component: "ScopeViolation"
  module: "approval"
  status: planned
  priority: high
  dependencies:
    - "none"

  in_scope:
    - "File tool outside declared scope → `scope_violation` flagged in envelope"
    - "`run_command` side-effect on `src/auth.ts` (script) → caught by **git-diff oracle** → `scope_violation`"

  out_of_scope:
    - Changes to upstream components (none)
    - UI/frontend changes
    - Deployment pipeline configuration

  canonical_references:
    - module: ".pi/architecture/modules/approval.md"
    - acceptance_criteria: ".pi/architecture/modules/approval.md#acceptance-criteria"

  acceptance_criteria:
    - "File tool outside declared scope → `scope_violation` flagged in envelope"
    - "`run_command` side-effect on `src/auth.ts` (script) → caught by **git-diff oracle** → `scope_violation`"

  validators:
    - ci
    - tests
    - security
    - architecture
    - canonical

  implementation_notes: |
    Read .pi/architecture/modules/approval.md BEFORE implementing.
    All Acceptance Criteria in that file must be satisfied before this issue is closed.
    Component focus: ScopeViolation.
    CONCRETE IMPLEMENTATIONS MUST BE CREATED — interface stubs from the contract freeze
    are not sufficient. Each domain service/aggregate needs a concrete .impl.rs file.

  file_changes:
    - "create: src/approval/domain/"
    - "create: src/approval/application/"
    - "create: src/approval/infrastructure/"
    - "modify: src/approval/interfaces/"
    -     - "update: tests/unit/ (failing tests already generated — make them pass)"
---

# ISSUE-APPROVAL-5: Implement ScopeViolation — approval

## Intent

Implement **ScopeViolation** for the `approval` module.

> ⚠️ **Read before implementing:** `.pi/architecture/modules/approval.md`
> Every item in the **Acceptance Criteria** section of that file must be satisfied
> before this issue is closed — including adapter, mapper, and WireMock items.

## Architecture Context

- **Module:** approval
- **Component:** ScopeViolation
- **Status:** planned
- **Dependencies:** none

## In Scope (this component)

- File tool outside declared scope → `scope_violation` flagged in envelope
- `run_command` side-effect on `src/auth.ts` (script) → caught by **git-diff oracle** → `scope_violation`

## Acceptance Criteria (this issue)

These acceptance criteria must be satisfied before this issue can be closed:

| # | Criterion | Verify In |
|---|-----------|-----------|
| 11 | File tool outside declared scope → `scope_violation` flagged in envelope | integration test |
| 12 | `run_command` side-effect on `src/auth.ts` (script) → caught by **git-diff oracle** → `scope_violation` | integration test |

## Full Module Acceptance Criteria

> All items below must pass before the **epic** is closed.
> Items may be split across multiple issues — verify your component's items before creating the MR.

| # | Component | Criterion | Verify In |
|---|-----------|-----------|-----------|
| 1 | ExecutionIntent | `from_node` on sealed node produces canonical intent; `canonical_bytes` deterministic (property test) | unit test |
| 2 | IntentHash | Same tool+intent+scope → same hash; any byte change → different hash | unit test |
| 3 | ApprovalRecord | Serde round-trip preserves all fields; status transitions valid (Pending → Consumed/Expired/Superseded) | unit test |
| 4 | ApprovalService | approve → dispatch identical intent → executes; envelope contains signed `ApprovalRecorded` | integration test |
| 5 | ApprovalService | approve → intent mutated (simulated upstream change / tampered state) → **HALT before dispatch**, `IntentMismatch`, tool never called (spy tool asserts) | integration test |
| 6 | ApprovalService | Cross-process: pause in A, tamper persisted intent in state file, resume in B → halted, re-approval required | integration test |
| 7 | ApprovalService | Retry path: failed approved node retries with same intent → per-attempt verify passes; a failed attempt does NOT consume; consume happens once on terminal outcome | integration test |
| 8 | ApprovalService | Replay: consumed approval replayed against same step → rejected (single-use + nonce) | integration test |
| 9 | ApprovalService | TTL: approval expires between approve and dispatch → `Invalid(Expired)`, no dispatch | unit test |
| 10 | ApprovalService | Migration: persisted `approved` without records → invalidated on hydrate, re-approval required | integration test |
| 11 | ScopeViolation | File tool outside declared scope → `scope_violation` flagged in envelope | integration test |
| 12 | ScopeViolation | `run_command` side-effect on `src/auth.ts` (script) → caught by **git-diff oracle** → `scope_violation` | integration test |
| 13 | DecisionContext | approve with decision_context → envelope contains context ref + summary; full payload opt-in | integration test |
| 14 | DecisionContext | api_key inside decision_context → redacted in summary (SpanPrivacy reuse) | unit test |
| 15 | ApprovalError | All variants, `Display`, `is_retriable()` (IntentMismatch → non-retriable) | unit test |

## Implementation Sequence (from module doc)

- Read .pi/architecture/modules/approval.md
- Implement entities and interfaces
- Implement infrastructure (adapter, mapper, repository)
- Implement use case
- Write unit + integration tests
- Run validators
- Create MR

## Implementation

> **Agent instructions:**
> 1. Open `.pi/architecture/modules/approval.md` — read the full Acceptance Criteria table
> 2. Identify which rows are your responsibility for **ScopeViolation**
> 3. Create concrete implementation files (`.impl.rs`) in `src/approval/` — the interface stubs from the contract freeze are NOT enough
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
