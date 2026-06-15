## Intent

Create engine/tests/ directory with cross-module integration tests. Currently all tests are inline or module-specific; critical end-to-end paths like plan→execute→audit are never tested with real (non-mocked) implementations.

## Epic
EPIC-003: Testing Hardening (Milestone #1)

## In Scope
- Create engine/tests/ directory
- Integration 1 — plan_to_execute: Full pipeline from UserIntent to TaskResult collection
- Integration 2 — budget_enforcement: Budget exhaustion triggers cancellation mid-execution
- Integration 3 — audit_trail: Full execution produces complete audit envelope
- Add #[cfg(feature = "integration")] gate for slow tests
- Use cargo test --test integration_* for CI separation

## Out of Scope
- Concurrent-safety tests (ISSUE-001)
- Live LLM API tests (ISSUE-003)

## Acceptance Criteria
- [ ] engine/tests/ directory exists with 3 integration test files
- [ ] plan_to_execute: UserIntent → valid TaskResult end-to-end
- [ ] budget_enforcement: exhausted budget prevents execution start
- [ ] audit_trail: full execution produces signed audit envelope
- [ ] All 3 tests pass when run with cargo test --test *integration*
- [ ] Tests are gated behind #[cfg(feature = "integration")]

## Implementation Notes
- Use real implementations wired together, not mocks
- This may require exposing factory constructors that wire real dependencies
- Use temp directories for state persistence to avoid cross-test contamination
- Use #[serial_test::serial] if tests share global state

## Files Changed
- create: engine/tests/plan_to_execute_integration.rs
- create: engine/tests/budget_enforcement_integration.rs
- create: engine/tests/audit_trail_integration.rs
- create: engine/tests/mod.rs
- modify: engine/Cargo.toml (add [dev-dependencies] for test helpers)

## Validators Required
- ci, tests, architecture
