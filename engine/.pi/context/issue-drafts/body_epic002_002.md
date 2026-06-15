## Intent

The execution/ module is a stub containing only ExecutionError, overlapping with execution_engine/ which has the full implementation. Merge the error into execution_engine and remove the execution/ module.

## Epic
EPIC-002: Architecture & Code Quality (Milestone #3)

## In Scope
- Merge execution::domain::ExecutionError into execution_engine::domain::error.rs
- Update error.rs (root) import
- Update all cross-module imports
- Remove engine/src/execution/ directory
- Verify cargo build and full test suite

## Out of Scope
- Classifier moves (ISSUE-001)
- Compiler warnings (ISSUE-003)
- is_retriable() (ISSUE-004)

## Acceptance Criteria
- [ ] execution/ directory removed
- [ ] ExecutionError available from execution_engine::domain::ExecutionError
- [ ] All 889+ tests pass
- [ ] cargo build produces zero errors

## Implementation Notes
- Option A (preferred): Merge ExecutionError variants into execution_engine::domain::error.rs and add #[deprecated] re-export aliases for one release cycle
- Check all files that use use crate::execution:: pattern

## Files Changed
- modify: src/error.rs (update import path)
- modify: src/execution_engine/domain/error.rs (absorb ExecutionError variants)
- remove: src/execution/ directory
- modify: src/lib.rs (remove pub mod execution)

## Validators Required
- ci, tests, architecture
