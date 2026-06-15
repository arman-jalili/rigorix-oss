## Intent

Add concurrent-safety tests for the 5 modules that use RwLock, Mutex, AtomicU32, tokio broadcast channels, and CancellationToken. Currently zero tests exercise concurrent access patterns.

## Epic
EPIC-003: Testing Hardening (Milestone #1)

## In Scope
- budget_tracking: 10 parallel reserve() tasks, verify atomic counters consistent
- dag_engine: Simultaneous graph mutations, verify RwLock doesn't race
- execution_engine: Race on pause+resume+abort, verify state machine consistency
- state_persistence: Concurrent read/write on same execution state, verify atomic write-rename isolation
- event_system: Publish+subscribe under load (100+ concurrent publishers), verify no dropped events
- Use tokio::try_join! and tokio::spawn patterns
- Evaluate loom for lock-free correctness checking (document decision)

## Out of Scope
- Integration test suite (ISSUE-002)
- Live LLM API tests (ISSUE-003)
- General unit test coverage

## Acceptance Criteria
- [ ] 10 parallel budget reservations complete without counter drift
- [ ] Simultaneous DAG mutations don't panic or deadlock
- [ ] Pause/resume/abort race completes without inconsistent state
- [ ] 100 concurrent event publishers produce no dropped events
- [ ] All new tests pass consistently (no flakiness over 10 runs)

## Implementation Notes
- Use #[tokio::test(flavor = "multi_thread")] for true concurrent execution
- Each test should be run 10 times in CI to verify no flakiness
- Use tokio::sync::Barrier for synchronizing parallel start
- Add #[cfg(feature = "stress-tests")] for slow concurrency tests

## Files Changed
- create: engine/src/budget_tracking/concurrency_tests.rs
- create: engine/src/dag_engine/concurrency_tests.rs
- create: engine/src/execution_engine/concurrency_tests.rs
- create: engine/src/state_persistence/concurrency_tests.rs
- create: engine/src/event_system/concurrency_tests.rs

## Validators Required
- ci, tests, operations
