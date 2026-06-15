---
guardian_issue:
  id: EPIC-002-ISSUE-003
  title: "Fix 24 compiler warnings (H-01)"
  epic: "Architecture & Code Quality"
  epic_id: EPIC-002
  status: planned
  priority: medium
  created_at: "2026-06-15"

  intent: |
    Fix 24 compiler warnings across the project. 15 are auto-fixable via cargo fix.
    Manually review remaining ~9 warnings for dead code, unused variables, and
    unnecessary mut. Enable #![deny(warnings)] in CI after all warnings are fixed.

  dependencies:
    - name: "EPIC-002-ISSUE-001, EPIC-002-ISSUE-002"
      type: internal
      note: "Do after architecture reorg to avoid fixing warnings that get moved"

  in_scope:
    - Run `cargo fix --lib -p rigorix` to auto-fix ~15 warnings
    - Manually review remaining ~9 warnings
    - Remove dead code or add #[allow(dead_code)] with justification comments
    - Add `#![deny(warnings)]` to CI lint stage
    - Verify cargo build produces zero warnings
    - Verify cargo clippy -- -D warnings passes

  out_of_scope:
    - Architecture reorg (ISSUE-001, ISSUE-002)
    - is_retriable() (ISSUE-004)

  affected_layers:
    all: project-wide cleanup

  acceptance_criteria:
    - "cargo build produces zero warnings"
    - "cargo fix --check produces zero warnings"
    - "CI lint stage includes -D warnings"
    - "All existing tests still pass"

  validators:
    - ci
    - tests

  implementation_notes: |
    - Current warnings (from cargo build output):
      - 5 unused imports (HashMap, PathBuf, GeneratorError, etc.)
      - 3 unused variables (config, node_id, gen_output)
      - 4 unnecessary mut qualifiers
      - 2 values assigned but never read
      - 1 unused struct (LlmBudgetReservationImpl)
      - 1 unused associated function (new)
      - 1 unused field (total_retries, usage_hint)
      - 1 unused method (notify_progress)
      - 1 unused function (create_test_symbol)
      - Remaining from cargo build output
    - For unused structs/functions that are reserved for future use, add
      #[allow(dead_code)] with a comment explaining why
    - cargo fix handles most import and mut issues automatically

  file_changes:
    - project-wide: cargo fix auto-fixes ~15 warnings
    - manual review: ~9 files with remaining warnings
    - "modify: CI scripts to add -D warnings"
---

# EPIC-002-ISSUE-003: Fix 24 compiler warnings (H-01)

## Intent

Clean up all compiler warnings so cargo build is clean and -D warnings can be enabled in CI.

## Dependencies

```
ISSUE-001, ISSUE-002 (architecture reorg) → ISSUE-003 (warning cleanup)
```

## In Scope

- cargo fix for auto-fixable warnings
- Manual review of remaining warnings
- -D warnings in CI

## Acceptance Criteria

| # | Criterion | Validator |
|---|-----------|-----------|
| 1 | `cargo build` zero warnings | CI |
| 2 | `cargo fix --check` zero warnings | CI |
| 3 | CI includes `-D warnings` | CI |
| 4 | All tests pass | Tests |
