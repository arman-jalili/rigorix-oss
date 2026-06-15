## Intent

Fix 24 compiler warnings across the project. 15 are auto-fixable via cargo fix. Manually review remaining ~9 warnings for dead code, unused variables, and unnecessary mut. Enable #![deny(warnings)] in CI after all warnings are fixed.

## Epic
EPIC-002: Architecture & Code Quality (Milestone #3)

## In Scope
- Run cargo fix --lib -p rigorix to auto-fix ~15 warnings
- Manually review remaining ~9 warnings
- Remove dead code or add #[allow(dead_code)] with justification comments
- Add #![deny(warnings)] to CI lint stage
- Verify cargo build produces zero warnings
- Verify cargo clippy -- -D warnings passes

## Out of Scope
- Architecture reorg (ISSUE-001, ISSUE-002)
- is_retriable() (ISSUE-004)

## Acceptance Criteria
- [ ] cargo build produces zero warnings
- [ ] cargo fix --check produces zero warnings
- [ ] CI lint stage includes -D warnings
- [ ] All existing tests still pass

## Implementation Notes
- Current warnings include: 5 unused imports, 3 unused variables, 4 unnecessary mut, 2 values assigned but never read, 1 unused struct, 1 unused function, 1 unused field
- For unused structs/functions reserved for future use, add #[allow(dead_code)] with justification comments

## Validators Required
- ci, tests
