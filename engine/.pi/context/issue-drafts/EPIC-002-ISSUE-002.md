---
guardian_issue:
  id: EPIC-002-ISSUE-002
  title: "Merge execution stub into execution_engine (H-05)"
  epic: "Architecture & Code Quality"
  epic_id: EPIC-002
  status: planned
  priority: high
  created_at: "2026-06-15"

  intent: |
    The execution/ module is a stub containing only ExecutionError, overlapping
    with execution_engine/ which has the full implementation. Merge the error
    into execution_engine and remove the execution/ module.

  dependencies:
    - name: "EPIC-002-ISSUE-001"
      type: internal
      note: "Do after ISSUE-001 to avoid overlapping move conflicts"

  in_scope:
    - Merge execution::domain::ExecutionError into execution_engine::domain::error.rs
    - Update error.rs (root) import from `use crate::execution::domain::ExecutionError` to `use crate::execution_engine::domain::ExecutionError`
    - Update all cross-module imports referencing execution::ExecutionError
    - Remove engine/src/execution/ directory
    - Verify cargo build and full test suite

  out_of_scope:
    - Classifier moves (ISSUE-001)
    - Compiler warnings (ISSUE-003)
    - is_retriable() (ISSUE-004)

  affected_layers:
    domain:
      - "Modified: execution_engine/domain/error.rs — absorb ExecutionError variants"
    all:
      - "Remove: src/execution/ directory"
      - "Modified: src/error.rs — update import"
      - "Modified: all files importing crate::execution::*"

  acceptance_criteria:
    - "execution/ directory removed"
    - "ExecutionError available from execution_engine::domain::ExecutionError"
    - "All 889+ tests pass"
    - "cargo build produces zero errors"

  validators:
    - ci
    - tests
    - architecture

  implementation_notes: |
    - Option A (preferred): Merge ExecutionError variants into execution_engine::domain::error.rs
      and add #[deprecated] re-export aliases at the old path for one release cycle
    - Option B (minimal): Add #[deprecated = "Use execution_engine instead"] on the execution module
      and remove in next version. User prefers Option A.
    - Check all files that use `use crate::execution::` pattern via grep
    - execution/domain/error.rs content is a small ExecutionError — likely just a few variants

  file_changes:
    - "modify: src/error.rs (update import path)"
    - "modify: src/execution_engine/domain/error.rs (absorb ExecutionError variants)"
    - "remove: src/execution/ directory (mod.rs, domain/)"
    - "modify: src/lib.rs (remove pub mod execution)"
    - "search-and-replace: update all `use crate::execution::` imports across project"
---

# EPIC-002-ISSUE-002: Merge execution stub into execution_engine (H-05)

## Intent

Eliminate the duplicate execution/ module by merging its content into execution_engine/.

## Dependencies

```
ISSUE-001 (classifier moves) → ISSUE-002 (execution merge)
```

## In Scope

- Absorb ExecutionError into execution_engine
- Update root error.rs import
- Remove execution/ directory
- Update all cross-module imports

## Acceptance Criteria

| # | Criterion | Validator |
|---|-----------|-----------|
| 1 | `src/execution/` directory removed | Architecture |
| 2 | `ExecutionError` accessible from `execution_engine` | CI |
| 3 | All 889+ tests pass | Tests |
| 4 | `cargo build` zero errors | CI |
