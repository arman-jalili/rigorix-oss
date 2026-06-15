## Intent

Currently CoreOrchestratorError::is_retriable() only returns true for Io and Http (5xx) errors. Domain errors like CancellationError, LlmBudgetError, or ExecutionError could succeed on retry but are hard-classified as non-retriable. Add is_retriable() to each domain error trait and delegate from the root error.

## Epic
EPIC-002: Architecture & Code Quality (Milestone #3)

## In Scope
- Add is_retriable() method to each domain error type (14 modules)
- CoreOrchestratorError::is_retriable() delegates to inner error's method
- Sensible defaults per module (e.g., LlmError -> retriable, CycleDetected -> not retriable)
- Update existing is_retriable tests
- Add tests for new domain-level retriable logic

## Out of Scope
- Architecture reorg (ISSUE-001, ISSUE-002)
- Compiler warnings (ISSUE-003)

## Acceptance Criteria
- [ ] Each domain error enum has an is_retriable() method
- [ ] CoreOrchestratorError::is_retriable() delegates correctly
- [ ] All existing retriable tests still pass
- [ ] New tests cover each domain error's retriable semantics

## Implementation Notes
- Add is_retriable() as a method on each error enum, not a separate trait
- Root error delegates: CoreOrchestratorError::is_retriable() -> inner.is_retriable()
- Use matches! macro for simple pattern matching

## Files Changed
- modify: src/error.rs (delegate is_retriable to inner errors)
- modify: src/*/domain/error.rs (add is_retriable to all 14 domain error types)

## Validators Required
- ci, tests, architecture
