---
guardian_issue:
  id: EPIC-003-ISSUE-001
  title: "Concurrent-safety tests (C-03)"
  epic: "Testing Hardening"
  epic_id: EPIC-003
  status: planned
  priority: critical
  created_at: "2026-06-15"

  intent: |
    Add concurrent-safety tests for the 5 modules that use RwLock, Mutex,
    AtomicU32, tokio broadcast channels, and CancellationToken. Currently zero
    tests exercise concurrent access patterns. Add tokio::spawn-based tests
    that verify correctness under contention.

  dependencies:
    - name: "EPIC-002"
      type: internal
      note: "Architecture cleanup must be complete before adding tests to moved code"

  in_scope:
    - **budget_tracking:** 10 parallel `reserve()` tasks, verify atomic counters consistent after all commit/rollback
    - **dag_engine:** Simultaneous graph mutations, verify RwLock doesn't race
    - **execution_engine:** Race on pause+resume+abort, verify state machine consistency
    - **state_persistence:** Concurrent read/write on same execution state, verify atomic write-rename isolation
    - **event_system:** Publish+subscribe under load (100+ concurrent publishers), verify no dropped events
    - Use `tokio::try_join!` and `tokio::spawn` patterns
    - Evaluate `loom` for lock-free correctness checking (document decision)

  out_of_scope:
    - Integration test suite (ISSUE-002)
    - Live LLM API tests (ISSUE-003)
    - General unit test coverage

  affected_layers:
    testing:
      - "New: engine/src/budget_tracking/concurrency_tests.rs"
      - "New: engine/src/dag_engine/concurrency_tests.rs"
      - "New: engine/src/execution_engine/concurrency_tests.rs"
      - "New: engine/src/state_persistence/concurrency_tests.rs"
      - "New: engine/src/event_system/concurrency_tests.rs"

  acceptance_criteria:
    - "10 parallel budget reservations complete without counter drift"
    - "Simultaneous DAG mutations don't panic or deadlock"
    - "Pause/resume/abort race completes without inconsistent state"
    - "100 concurrent event publishers produce no dropped events"
    - "All new tests pass consistently (no flakiness observed over 10 runs)"

  validators:
    - ci
    - tests
    - operations

  implementation_notes: |
    - Use `#[tokio::test(flavor = "multi_thread")]` for true concurrent execution
    - Each test should be run 10 times in CI to verify no flakiness
    - Use tokio::sync::Barrier for synchronizing parallel start
    - For budget_tracking: spawn 10 tasks that each reserve→commit, verify final
      counts match expected (e.g., 10 calls used)
    - For event_system: spawn 100 tokio tasks publishing simultaneously, one
      subscriber, verify all events received
    - Consider loom only if we find a race condition; start with tokio test
    - Add #[cfg(feature = "stress-tests")] for slow concurrency tests

  file_changes:
    - "create: engine/src/budget_tracking/concurrency_tests.rs"
    - "create: engine/src/dag_engine/concurrency_tests.rs"
    - "create: engine/src/execution_engine/concurrency_tests.rs"
    - "create: engine/src/state_persistence/concurrency_tests.rs"
    - "create: engine/src/event_system/concurrency_tests.rs"
---

# EPIC-003-ISSUE-001: Concurrent-safety tests (C-03)

## Intent

Add concurrent-safety tests for 5 modules that use shared-memory concurrency primitives.

## Dependencies

```
EPIC-002 (architecture cleanup) → EPIC-003-ISSUE-001
```

## In Scope

- 5 concurrency test suites using `tokio::spawn` and `try_join!`
- Each verified for no flakiness over 10 runs
- Using `#[tokio::test(flavor = "multi_thread")]`

## Acceptance Criteria

| # | Criterion | Validator |
|---|-----------|-----------|
| 1 | 10 parallel budget reservations complete without drift | Tests |
| 2 | DAG mutations don't panic under concurrent access | Tests |
| 3 | Pause/resume/abort races don't produce inconsistent state | Tests |
| 4 | 100 concurrent publishers don't drop events | Tests |
| 5 | All tests pass consistently over 10 runs | Operations |
