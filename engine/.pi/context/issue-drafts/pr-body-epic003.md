## feat: EPIC-003 — Testing Hardening (issues #215-#217)

### Issues Implemented

| # | Title | Gap | Scope |
|---|-------|-----|-------|
| #215 | Concurrent-safety tests | C-03 | moderate |
| #216 | Cross-module integration test suite | H-02 | moderate |
| #217 | Live LLM API integration tests | H-03 | moderate |

### Changes

**#215 — Concurrent-safety tests**
- `budget_tracking/concurrency_tests.rs`: 2 tests
  - 5 parallel reserve+commit — verifies no counter drift
  - 12 concurrent callers competing for 5 budget slots — verifies exhaustion

**#216 — Integration test suite**
- `engine/tests/` directory with 3 files (5 tests total):
  - `plan_to_execute_integration`: domain type construction tests
  - `budget_enforcement_integration`: budget exhaustion prevents execution
  - `audit_trail_integration`: HMAC signature verification in audit envelopes

**#217 — Live LLM API tests**
- Added `[features] live-tests = []` to Cargo.toml
- `live_classifier_tests.rs` behind `#[cfg(feature = "live-tests")]`
- Tests ClaudeClassifier and OpenAIClassifier against real APIs
- Gracefully skip when API keys not set (env-only, never hardcoded)

### Validation
- ✅ `cargo build` — zero errors
- ✅ `cargo test` — 915/915 passed (904 unit + 5 integration + 6 doc)
- ✅ `cargo test --features live-tests` — compiles and runs (API tests skip without keys)
- ✅ `cargo fmt` — compliant

### Security Conditions Met
- [x] API keys from environment variables only (CLAUDE_API_KEY, OPENAI_API_KEY)
- [x] No hardcoded secrets in source files

### Tracking
- Closes: #215, #216, #217
- Epic: EPIC-003 (Milestone #1)
- Tracking: #220
