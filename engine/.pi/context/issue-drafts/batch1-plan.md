# Batch 1 Implementation Plan: EPIC-002 Architecture Cleanup

**Branch:** `feat/epic002-arch-cleanup`
**Issues:** #207, #208, #209, #210
**Gaps:** H-04, H-05, H-01, H-06

## Implementation Order

```
#207 (classifier moves) → #208 (execution merge) → #209 (warnings) → #210 (retriable)
```

## Issue #207 — Move classifiers out of domain layer

### Changes
- Move: `planning/domain/claude_classifier.rs` → `planning/infrastructure/claude_classifier.rs`
- Move: `planning/domain/openai_classifier.rs` → `planning/infrastructure/openai_classifier.rs`
- Move: `planning/domain/mock_classifier.rs` → `planning/application/mock_classifier.rs`
- Move: `planning/domain/mock_extractor.rs` → `planning/application/mock_extractor.rs`
- Update: `planning/domain/mod.rs`, `planning/infrastructure/mod.rs`, `planning/application/mod.rs`
- Update: `planning/tests.rs` and any other files importing moved modules

### Validation
- `cargo build` — zero errors
- `cargo test` — all 889+ tests pass

## Issue #208 — Merge execution stub into execution_engine

### Changes
- Merge `execution::domain::ExecutionError` into `execution_engine::domain::error.rs`
- Update `src/error.rs` import
- Remove `src/execution/` directory
- Update `src/lib.rs`
- Update any cross-module imports

### Validation
- `cargo build` — zero errors
- All tests pass

## Issue #209 — Fix 24 compiler warnings

### Changes
- Run `cargo fix --lib -p rigorix` (auto-fix ~15)
- Manual review of remaining ~9 warnings
- Add `#![deny(warnings)]` if not already present

### Validation
- `cargo build` — zero warnings
- `cargo clippy -- -D warnings` — passes

## Issue #210 — Per-module is_retriable() delegation

### Changes
- Add `is_retriable()` to all 14 domain error enums
- Update `CoreOrchestratorError::is_retriable()` to delegate
- Add tests for new retriable semantics

### Validation
- `cargo test` — all tests pass including new retriable tests
- `cargo build` — zero errors
