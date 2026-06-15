---
guardian_issue:
  id: EPIC-003-ISSUE-002
  title: "Cross-module integration test suite (H-02)"
  epic: "Testing Hardening"
  epic_id: EPIC-003
  status: planned
  priority: high
  created_at: "2026-06-15"

  intent: |
    Create engine/tests/ directory with cross-module integration tests. Currently
    all tests are inline or module-specific; critical end-to-end paths like
    plan→execute→audit are never tested with real (non-mocked) implementations.

  dependencies:
    - name: "EPIC-002"
      type: internal
      note: "Architecture cleanup must be complete so integration tests use correct module paths"

  in_scope:
    - Create `engine/tests/` directory
    - **Integration 1 — plan_to_execute:** Full pipeline from UserIntent →
      PlanningResult → TaskGraph → execution → TaskResult collection. Wire real
      implementations: DAG Engine, Execution Engine, Planning Pipeline, Template
      System, Event System.
    - **Integration 2 — budget_enforcement:** Budget exhaustion triggers cancellation
      mid-execution. Verify: budget pre-check fails, execution doesn't start,
      LlmBudgetError propagated correctly.
    - **Integration 3 — audit_trail:** Full execution produces complete audit envelope.
      Verify: correct event sequence, HMAC signature present, audit queue delivery works.
    - Add `#[cfg(feature = "integration")]` gate for slow tests
    - Use `cargo test --test integration_*` for CI separation

  out_of_scope:
    - Concurrent-safety tests (ISSUE-001)
    - Live LLM API tests (ISSUE-003)

  affected_layers:
    testing:
      - "New: engine/tests/plan_to_execute_integration.rs"
      - "New: engine/tests/budget_enforcement_integration.rs"
      - "New: engine/tests/audit_trail_integration.rs"
      - "New: engine/tests/mod.rs"
    infrastructure:
      - "Possibly: new factory constructors for wiring real implementations"

  acceptance_criteria:
    - "engine/tests/ directory exists with 3 integration test files"
    - "plan_to_execute: UserIntent → valid TaskResult end-to-end"
    - "budget_enforcement: exhausted budget prevents execution start"
    - "audit_trail: full execution produces signed audit envelope"
    - "All 3 tests pass when run with `cargo test --test *integration*`"
    - "Tests are gated behind #[cfg(feature = \"integration\")]"

  validators:
    - ci
    - tests
    - architecture

  implementation_notes: |
    - Use real implementations wired together, not mocks
    - This may require exposing factory constructors that wire real dependencies
      (e.g., PlanningPipelineFactory::create_default())
    - Each test should create its own service graph and drop it at end
    - Use temp directories for state persistence to avoid cross-test contamination
    - Integration tests go in engine/tests/, using crate name as `extern crate rigorix`
    - Follow the AAA pattern (Arrange-Act-Assert) with clear sections
    - Use #[serial_test::serial] if tests share global state

  file_changes:
    - "create: engine/tests/plan_to_execute_integration.rs"
    - "create: engine/tests/budget_enforcement_integration.rs"
    - "create: engine/tests/audit_trail_integration.rs"
    - "create: engine/tests/mod.rs"
    - "modify: engine/Cargo.toml (add [dev-dependencies] for test helpers)"
---

# EPIC-003-ISSUE-002: Cross-module integration test suite (H-02)

## Intent

Create a top-level integration test directory and 3 end-to-end tests that exercise real service wiring.

## Dependencies

```
EPIC-002 (architecture cleanup) → EPIC-003-ISSUE-002
```

## In Scope

- `engine/tests/` directory
- 3 integration tests: plan→execute, budget enforcement, audit trail
- Feature-gated behind `#[cfg(feature = "integration")]`
- Real (non-mocked) service implementations

## Acceptance Criteria

| # | Criterion | Validator |
|---|-----------|-----------|
| 1 | `engine/tests/` exists with 3 files | CI |
| 2 | plan→execute produces valid TaskResult | Tests |
| 3 | Budget exhaustion prevents execution | Tests |
| 4 | Full execution produces signed audit envelope | Tests |
