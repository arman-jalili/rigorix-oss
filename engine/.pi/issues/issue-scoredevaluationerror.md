---
guardian_issue:
  id: "ISSUE-SCORED-EVALUATION-9"
  epic: "TBD"
  component: "ScoredEvaluationError"
  module: "scored-evaluation"
  status: planned
  priority: high
  dependencies:
    - "none"

  in_scope:
    - "All 7 variants, `Display` impl, `is_retriable()` classification correct"

  out_of_scope:
    - Changes to upstream components (none)
    - UI/frontend changes
    - Deployment pipeline configuration

  canonical_references:
    - module: ".pi/architecture/modules/scored-evaluation.md"
    - acceptance_criteria: ".pi/architecture/modules/scored-evaluation.md#acceptance-criteria"

  acceptance_criteria:
    - "All 7 variants, `Display` impl, `is_retriable()` classification correct"

  validators:
    - ci
    - tests
    - security
    - architecture
    - canonical

  implementation_notes: |
    Read .pi/architecture/modules/scored-evaluation.md BEFORE implementing.
    All Acceptance Criteria in that file must be satisfied before this issue is closed.
    Component focus: ScoredEvaluationError.
    CONCRETE IMPLEMENTATIONS MUST BE CREATED — interface stubs from the contract freeze
    are not sufficient. Each domain service/aggregate needs a concrete .impl.rs file.

  file_changes:
    - "create: src/scored-evaluation/domain/"
    - "create: src/scored-evaluation/application/"
    - "create: src/scored-evaluation/infrastructure/"
    - "modify: src/scored-evaluation/interfaces/"
    -     - "update: tests/unit/ (failing tests already generated — make them pass)"
---

# ISSUE-SCORED-EVALUATION-9: Implement ScoredEvaluationError — scored-evaluation

## Intent

Implement **ScoredEvaluationError** for the `scored-evaluation` module.

> ⚠️ **Read before implementing:** `.pi/architecture/modules/scored-evaluation.md`
> Every item in the **Acceptance Criteria** section of that file must be satisfied
> before this issue is closed — including adapter, mapper, and WireMock items.

## Architecture Context

- **Module:** scored-evaluation
- **Component:** ScoredEvaluationError
- **Status:** planned
- **Dependencies:** none

## In Scope (this component)

- All 7 variants, `Display` impl, `is_retriable()` classification correct

## Acceptance Criteria (this issue)

These acceptance criteria must be satisfied before this issue can be closed:

| # | Criterion | Verify In |
|---|-----------|-----------|
| 11 | All 7 variants, `Display` impl, `is_retriable()` classification correct | unit test |

## Full Module Acceptance Criteria

> All items below must pass before the **epic** is closed.
> Items may be split across multiple issues — verify your component's items before creating the MR.

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

## Implementation Sequence (from module doc)

- Read .pi/architecture/modules/scored-evaluation.md
- Implement entities and interfaces
- Implement infrastructure (adapter, mapper, repository)
- Implement use case
- Write unit + integration tests
- Run validators
- Create MR

## Implementation

> **Agent instructions:**
> 1. Open `.pi/architecture/modules/scored-evaluation.md` — read the full Acceptance Criteria table
> 2. Identify which rows are your responsibility for **ScoredEvaluationError**
> 3. Create concrete implementation files (`.impl.rs`) in `src/scored-evaluation/` — the interface stubs from the contract freeze are NOT enough
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
