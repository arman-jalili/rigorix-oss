## feat: implement EPIC-002 — Architecture & Code Quality

### Issues Implemented

| # | Title | Gap | Scope |
|---|-------|-----|-------|
| #207 | Move classifiers out of domain layer | H-04 | moderate |
| #208 | Merge execution stub into execution_engine | H-05 | moderate |
| #209 | Fix 24 compiler warnings | H-01 | simple |
| #210 | Per-module is_retriable() delegation | H-06 | moderate |

### Changes

**#207 — Classifier moves (Clean Architecture)**
- Moved `ClaudeClassifier`, `OpenAIClassifier` → `planning/infrastructure/`
- Moved `MockClassifier`, `MockParameterExtractor` → `planning/application/`
- `planning/domain/` now contains only trait definitions and entities

**#208 — Execution module merge**
- Removed `src/execution/` legacy stub module
- Updated `error.rs` to import `ExecutionError` from `execution_engine::domain`
- Updated `lib.rs` declaration and doc comments

**#209 — Compiler warnings**
- Fixed all 24 warnings via `cargo fix` + manual review
- Zero warnings on `cargo build`
- Ready for `-D warnings` CI enforcement

**#210 — Per-module retriable logic**
- Added `is_retriable()` to all 14 domain error enums
- `CoreOrchestratorError::is_retriable()` delegates to inner error
- Sensible defaults: LLM/network errors retriable, domain violations not

### Validation
- ✅ `cargo build` — zero errors, zero warnings
- ✅ `cargo test` — 889/889 passed
- ✅ `cargo fmt` — compliant
- ⚠️ `cargo clippy` — 57 pre-existing warnings (not introduced by this PR)

### Tracking
- Closes: #207, #208, #209, #210
- Epic: EPIC-002 (Milestone #3)
- Tracking: #218
