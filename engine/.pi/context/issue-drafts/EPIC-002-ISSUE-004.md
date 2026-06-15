---
guardian_issue:
  id: EPIC-002-ISSUE-004
  title: "Per-module is_retriable() delegation (H-06)"
  epic: "Architecture & Code Quality"
  epic_id: EPIC-002
  status: planned
  priority: medium
  created_at: "2026-06-15"

  intent: |
    Currently CoreOrchestratorError::is_retriable() only returns true for Io
    and Http (5xx) errors. Domain errors like CancellationError,
    LlmBudgetError, or ExecutionError could succeed on retry but are hard-
    classified as non-retriable. Add is_retriable() to each domain error trait
    and delegate from the root error.

  dependencies:
    - name: "EPIC-002-ISSUE-002"
      type: internal
      note: "ExecutionError import path changes after execution merge"

  in_scope:
    - Add `is_retriable()` method to each domain error type:
      - DagError, PlanningError, EnforcementError, LlmBudgetError
      - ExecutionError, ToolError, RepoEngineError, ConfigurationError
      - CancellationError, EventSystemError, AuditError, StateError
      - TemplateError, FailureClassificationError
    - CoreOrchestratorError::is_retriable() delegates to inner error's method
    - Sensible defaults per module:
      - DagError::CycleDetected → not retriable
      - LlmBudgetError::MaxCallsExceeded → not retriable
      - PlanningError::LlmError → retriable
      - ExecutionError::NodeExecutionFailed → retriable
      - etc.
    - Update existing is_retriable tests
    - Add tests for new domain-level retriable logic
    - Update docs in error handling module

  out_of_scope:
    - Architecture reorg (ISSUE-001, ISSUE-002)
    - Compiler warnings (ISSUE-003)

  affected_layers:
    domain:
      - "Modified: all 14 domain error enums — add is_retriable()"
    application:
      - "Modified: src/error.rs — delegate to inner error"

  acceptance_criteria:
    - "Each domain error enum has an is_retriable() method"
    - "CoreOrchestratorError::is_retriable() delegates correctly"
    - "All existing retriable tests still pass"
    - "New tests cover each domain error's retriable semantics"

  validators:
    - ci
    - tests
    - architecture

  implementation_notes: |
    - Add is_retriable() as a method on each error enum, not a separate trait
      (simpler, avoids trait object complexity)
    - Example pattern:
      ```rust
      impl PlanningError {
          pub fn is_retriable(&self) -> bool {
              matches!(self, PlanningError::LlmError { .. } | PlanningError::TemplateNotFound { .. })
          }
      }
      ```
    - Root error delegates: CoreOrchestratorError::is_retriable() → self.0.is_retriable()
    - Use matches! macro for simple pattern matching

  file_changes:
    - "modify: src/error.rs (delegate is_retriable to inner errors)"
    - "modify: src/dag_engine/domain/error.rs (add is_retriable)"
    - "modify: src/planning/domain/error.rs"
    - "modify: src/enforcement/domain/error.rs"
    - "modify: src/budget_tracking/domain/error.rs"
    - "modify: src/execution_engine/domain/error.rs"
    - "modify: src/tools/domain/error.rs"
    - "modify: src/repo_engine/domain/error.rs"
    - "modify: src/configuration/domain/error.rs"
    - "modify: src/cancellation/domain/error.rs"
    - "modify: src/event_system/domain/error.rs"
    - "modify: src/audit/domain/error.rs"
    - "modify: src/state_persistence/domain/error.rs"
    - "modify: src/templates/domain/error.rs"
    - "modify: src/failure_classification/domain/error.rs"
    - "modify: src/error.rs tests (update is_retriable tests)"
---

# EPIC-002-ISSUE-004: Per-module is_retriable() delegation (H-06)

## Intent

Let each domain error self-declare which variants are retriable, then delegate from the root error.

## Dependencies

```
ISSUE-002 (execution merge) → ISSUE-004 (needs correct ExecutionError path)
```

## In Scope

- Add `is_retriable()` to all 14 domain error enums
- Delegate from CoreOrchestratorError
- Update tests

## Acceptance Criteria

| # | Criterion | Validator |
|---|-----------|-----------|
| 1 | Each domain error has `is_retriable()` | Architecture |
| 2 | Root error delegates correctly | Tests |
| 3 | All existing + new is_retriable tests pass | Tests |
