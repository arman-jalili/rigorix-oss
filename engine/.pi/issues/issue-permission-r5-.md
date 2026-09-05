---
guardian_issue:
  id: "ISSUE-SEQUENCE-POLICY-12"
  epic: "TBD"
  component: "Permission (R5)"
  module: "sequence-policy"
  status: planned
  priority: high
  dependencies:
    - "none"

  in_scope:
    - "`workspace_write` agent file-write to `.rigorix/**` denied by default permission config"

  out_of_scope:
    - Changes to upstream components (none)
    - UI/frontend changes
    - Deployment pipeline configuration

  canonical_references:
    - module: ".pi/architecture/modules/sequence-policy.md"
    - acceptance_criteria: ".pi/architecture/modules/sequence-policy.md#acceptance-criteria"

  acceptance_criteria:
    - "`workspace_write` agent file-write to `.rigorix/**` denied by default permission config"

  validators:
    - ci
    - tests
    - security
    - architecture
    - canonical

  implementation_notes: |
    Read .pi/architecture/modules/sequence-policy.md BEFORE implementing.
    All Acceptance Criteria in that file must be satisfied before this issue is closed.
    Component focus: Permission (R5).
    CONCRETE IMPLEMENTATIONS MUST BE CREATED — interface stubs from the contract freeze
    are not sufficient. Each domain service/aggregate needs a concrete .impl.rs file.

  file_changes:
    - "create: src/sequence-policy/domain/"
    - "create: src/sequence-policy/application/"
    - "create: src/sequence-policy/infrastructure/"
    - "modify: src/sequence-policy/interfaces/"
    -     - "update: tests/unit/ (failing tests already generated — make them pass)"
---

# ISSUE-SEQUENCE-POLICY-12: Implement Permission (R5) — sequence-policy

## Intent

Implement **Permission (R5)** for the `sequence-policy` module.

> ⚠️ **Read before implementing:** `.pi/architecture/modules/sequence-policy.md`
> Every item in the **Acceptance Criteria** section of that file must be satisfied
> before this issue is closed — including adapter, mapper, and WireMock items.

## Architecture Context

- **Module:** sequence-policy
- **Component:** Permission (R5)
- **Status:** planned
- **Dependencies:** none

## In Scope (this component)

- `workspace_write` agent file-write to `.rigorix/**` denied by default permission config

## Acceptance Criteria (this issue)

These acceptance criteria must be satisfied before this issue can be closed:

| # | Criterion | Verify In |
|---|-----------|-----------|
| 13 | `workspace_write` agent file-write to `.rigorix/**` denied by default permission config | integration test |

## Full Module Acceptance Criteria

> All items below must pass before the **epic** is closed.
> Items may be split across multiple issues — verify your component's items before creating the MR.

| # | Component | Criterion | Verify In |
|---|-----------|-----------|-----------|
| 1 | SequenceRule | TOML parse → rule with ordered predicates + action; serde round-trip preserves all fields | unit test |
| 2 | StepPredicate | Tool glob + param exact/glob/regex match correctly; non-matching params do not match | unit test |
| 3 | Matcher | Adjacent pair match; windowed match (gap ≤ window); out-of-window does not match | unit test |
| 4 | Matcher | Determinism property test: same ordered plan + rules → same match set | unit test (property) |
| 5 | SequencePolicyService | `evaluate_plan` finds remove-then-add pair in a runbook; returns later step id | unit test |
| 6 | Orchestrator (R2) | Runbook containing remove-then-add: later step is built `requires_approval=true`; run pauses; approve → executes; reject → skipped | integration test |
| 7 | Orchestrator (R2) | `deny` rule: later step fails with `SequencePolicyDenied`, tool never called (spy tool asserts) | integration test |
| 8 | MCP surface | `rigorix_validate_plan` on a plan with a matched sequence returns a structured finding before run | integration test |
| 9 | Execution Engine (R3) | Dynamic plan completes step A, then proposes B (would complete) → B promoted/denied per rule | integration test |
| 10 | Fail-closed | Corrupt rule config → plan refused, no steps execute | integration test |
| 11 | Fail-open-absent | No config file → run executes unchanged | integration test |
| 12 | Audit (R6) | Matched rule + promotion recorded in envelope events; decision summaries redact parameter values by default (SpanPrivacy pattern) | integration test |
| 13 | Permission (R5) | `workspace_write` agent file-write to `.rigorix/**` denied by default permission config | integration test |
| 14 | SequencePolicyError | All variants, `Display`, `is_retriable()` | unit test |

## Implementation Sequence (from module doc)

- Read .pi/architecture/modules/sequence-policy.md
- Implement entities and interfaces
- Implement infrastructure (adapter, mapper, repository)
- Implement use case
- Write unit + integration tests
- Run validators
- Create MR

## Implementation

> **Agent instructions:**
> 1. Open `.pi/architecture/modules/sequence-policy.md` — read the full Acceptance Criteria table
> 2. Identify which rows are your responsibility for **Permission (R5)**
> 3. Create concrete implementation files (`.impl.rs`) in `src/sequence-policy/` — the interface stubs from the contract freeze are NOT enough
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
