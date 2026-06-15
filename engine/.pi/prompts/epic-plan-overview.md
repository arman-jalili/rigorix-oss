# Epic Overview: Production Readiness

**Generated:** 2026-06-15
**Source:** Gap Ledger analysis — 3 Critical + 6 High gaps

## Goal
Close the 9 highest-severity gaps across observability, architecture, code quality, and testing to make Rigorix production-ready.

## Module Breakdown

| Module | Scope | Key Changes | Dependencies | Order |
|--------|-------|-------------|--------------|-------|
| **All 17 modules** | Moderate | Add `tracing` crate, instrument all service methods | None | EPIC-001 |
| **All 16 modules with HTTP** | Moderate | Centralized `HealthService`, add `/health` + `/metrics` | tracing infrastructure | EPIC-001 |
| **planning module** | Medium | Move ClaudeClassifier/OpenAIClassifier → infrastructure | None | EPIC-002 |
| **execution module** | Small | Merge stub into execution_engine | None | EPIC-002 |
| **error.rs** | Small | Per-module `is_retriable()` delegation | H-05 done first | EPIC-002 |
| **Project-wide** | Small | 24 compiler warnings → fix | None | EPIC-002 |
| **5 modules** | Medium | Concurrent-safety tests | EPIC-002 (clean arch) | EPIC-003 |
| **Project-wide** | Large | Integration test suite (`engine/tests/`) | EPIC-002 (clean arch) | EPIC-003 |
| **planning + template_generation** | Medium | Live LLM API integration tests | EPIC-002 (clean arch) | EPIC-003 |

## Cross-Module Risks

- **Tracing adds overhead:** Must use compile-time feature flag (`features = ["tracing"]`) to allow disabling in non-production builds
- **Health endpoint duplication:** Must avoid conflicting `/health` paths — resolve via centralized `HealthService` route registration
- **Architecture reorg breaks imports:** H-04 and H-05 will change module paths — must coordinate with CI to catch stale imports
- **Integration tests need real wiring:** Must construct real (not mocked) service chains; may need new factory constructors

## Epic Sequence

1. **[EPIC-001] Observability Foundation** — Tracing infrastructure + centralized health + metrics
2. **[EPIC-002] Architecture & Code Quality** — Fix module boundaries, warnings, retriable logic
3. **[EPIC-003] Testing Hardening** — Concurrency tests, integration tests, live API tests

---

## EPIC-001: Observability Foundation

### Goal
Add structured tracing to every service method, a centralized health aggregator, and Prometheus metrics endpoints so the system is observable in production.

### Issue Breakdown

1. **[Issue] Add `tracing` crate + instrumentation** — Scope: moderate
   - Add `tracing`, `tracing-subscriber`, `tracing-appender` to Cargo.toml
   - Add `#[tracing::instrument]` to every service method across all 17 modules
   - Add spans for: LLM API calls, retry decisions, DAG node transitions, budget reserve/commit, cancellation signal propagation
   - Wire tracing to EventBus for dual emission (structured logs + events)
   - Configure `TracingConfig` in Configuration module (level, format, output)

2. **[Issue] Centralized HealthService** — Scope: moderate
   - Create `common::health::HealthService` aggregator trait + implementation
   - Register all module health checks (budget status, circuit-breaker states, active executions, event bus stats)
   - Add `/health` endpoint to the HTTP layer that delegates to HealthService
   - Add `/health/ready` and `/health/live` endpoints for k8s-style probes

3. **[Issue] Prometheus /metrics endpoints** — Scope: moderate
   - Add `prometheus` crate to Cargo.toml
   - Define metrics: budget consumption rate, retry frequency, execution latency distribution, circuit-breaker state transitions, event bus throughput
   - Create `MetricsRegistry` that all modules register counters/gauges/histograms with
   - Add `/metrics` endpoint for Prometheus scraping

4. **[Issue] Add health endpoints to remaining 13 modules** — Scope: moderate
   - Add `HEALTH_PATH` + `HealthResponse` to: audit, budget_tracking, cancellation, configuration, enforcement, error_handling, event_system, failure_classification, planning, repo_engine, risk_gating, template_generation, tools
   - Each module reports: status (up/down), last activity timestamp, key metrics

### Validators Required
- `architecture-validator` — tracing instrumentation pattern, health endpoint design
- `operations-validator` — tracing overhead, health check semantics, metrics cardinality
- `security-validator` — metrics endpoint access control, tracing data leakage (no PII in spans)

### Estimated Scope
- Files: ~40-50 (Cargo.toml + new common module + updates to all 17 modules)
- Lines: ~2,500-3,500
- Effort: 1-2 weeks (Critical C-01 + C-02 combined)

---

## EPIC-002: Architecture & Code Quality

### Goal
Fix module boundary violations, eliminate stub modules, fix compiler warnings, and improve error retriability logic.

### Issue Breakdown

1. **[Issue] Move classifiers out of domain layer** — Scope: moderate — Component: planning
   - Move `ClaudeClassifier` → `planning/infrastructure/claude_classifier.rs`
   - Move `OpenAIClassifier` → `planning/infrastructure/openai_classifier.rs`
   - Move `MockClassifier` → `planning/application/mock_classifier.rs` or behind `#[cfg(test)]`
   - Move `MockParameterExtractor` → `planning/application/mock_extractor.rs` or behind `#[cfg(test)]`
   - Update all imports across codebase and tests
   - Verify: full test suite passes

2. **[Issue] Merge execution stub into execution_engine** — Scope: small — Component: all
   - Merge `execution::domain::ExecutionError` into `execution_engine::domain::error.rs`
   - Update `error.rs` import to use `execution_engine::domain::ExecutionError`
   - Remove the `execution/` module directory
   - Add `#[deprecated]` re-export if backward compatibility needed
   - Verify: build clean, all tests pass

3. **[Issue] Fix 24 compiler warnings** — Scope: simple — Component: project-wide
   - Run `cargo fix --lib -p rigorix` (auto-fixes ~15 warnings)
   - Manually review remaining ~9: remove dead code or add `#[allow(dead_code)]` with justification comments
   - Add `#![deny(warnings)]` to CI pipeline stage
   - Verify: `cargo build` produces zero warnings, `cargo clippy -- -D warnings` passes

4. **[Issue] Per-module is_retriable() delegation** — Scope: simple — Component: error.rs
   - Add `is_retriable()` method to each domain error trait (or a `Retriable` trait)
   - Let modules self-declare which variants are retriable:
     - `DagError::CycleDetected` → not retriable
     - `LlmBudgetError::MaxCallsExceeded` → not retriable (new budget needed)
     - `PlanningError::LlmError` → retriable
     - `ExecutionError::NodeExecutionFailed` → retriable
     - etc.
   - `CoreOrchestratorError::is_retriable()` delegates to inner error's method
   - Verify: all existing is_retriable tests pass with new behavior

### Validators Required
- `architecture-validator` — Clean Architecture boundary compliance, module removal impact
- `ci-validator` — CI lint stage must block on warnings
- `integration-validator` — Import path changes may break cross-module references

### Estimated Scope
- Files: ~15-20
- Lines: ~800-1,200
- Effort: 5-7 days

---

## EPIC-003: Testing Hardening

### Goal
Add concurrent-safety tests, cross-module integration tests, and live LLM API integration tests so the system's correctness is verified under realistic conditions.

### Issue Breakdown

1. **[Issue] Concurrent-safety tests (5 modules)** — Scope: moderate
   - **budget_tracking:** 10 parallel `reserve()` tasks with overlapping budgets, verify atomic counters are consistent
   - **dag_engine:** Simultaneous graph mutations, verify RwLock doesn't race
   - **execution_engine:** race on pause+resume+abort, verify state machine consistency
   - **state_persistence:** Concurrent read/write on same execution state, verify atomic write-rename isolation
   - **event_system:** Publish+subscribe under load (100+ concurrent publishers), verify no dropped events
   - Use `tokio::try_join!` and `tokio::spawn` patterns
   - Evaluate `loom` for lock-free correctness checking

2. **[Issue] Cross-module integration test suite** — Scope: moderate
   - Create `engine/tests/` directory
   - **Integration 1 — plan_to_execute:** Full pipeline from `UserIntent` → `PlanningResult` → `TaskGraph` → execution → `TaskResult` collection. Wire real implementations (no mocks) for DAG Engine, Execution Engine, Planning Pipeline, Template System, Event System.
   - **Integration 2 — budget_enforcement:** Budget exhaustion triggers cancellation mid-execution. Verify: budget pre-check fails, execution doesn't start, `LlmBudgetError` propagated.
   - **Integration 3 — audit_trail:** Full execution produces complete audit envelope. Verify: event sequence, HMAC signature, audit queue delivery.
   - Add `#[cfg(feature = "integration")]` gate for slow tests

3. **[Issue] Live LLM API integration tests** — Scope: moderate
   - Add `#[cfg(feature = "live-tests")]` feature flag
   - **ClaudeClassifier:** Test `classify_with_alternatives()` against real Anthropic API. Verify: structured JSON response, confidence in [0,1], token usage populated.
   - **OpenAIClassifier:** Test against real OpenAI API. Verify same contract.
   - **ClaudeTemplateGenerator:** Test `generate()` against real API. Verify: valid TOML output, template registers successfully.
   - **Error handling:** Mock reqwest client to test: timeout → `GeneratorError::LlmError`, 429 → retry, 500 → retry exhaustion.
   - Add to CI as optional manual stage (secrets required)

### Validators Required
- `test-validator` — Test coverage thresholds, integration test structure
- `operations-validator` — Live API test cost, timeout handling
- `security-validator` — API key handling in integration tests

### Estimated Scope
- Files: ~10-15
- Lines: ~1,500-2,500
- Effort: 1-2 weeks

---

## Total Epic Summary

| Epic | Gaps Addressed | Effort | Dependencies |
|------|---------------|--------|-------------|
| **EPIC-001** Observability Foundation | C-01, C-02 | 1-2 weeks | None |
| **EPIC-002** Architecture & Code Quality | H-01, H-04, H-05, H-06, M-05 | 5-7 days | None |
| **EPIC-003** Testing Hardening | C-03, H-02, H-03 | 1-2 weeks | EPIC-002 |

**Total estimated effort:** 3-5 weeks for all 9 gaps

**Recommended order:** EPIC-002 → EPIC-001 → EPIC-003 (fix architecture first, then add observability, then test)

### Pending Architecture Changes
- No architecture ADRs need changing — these are implementation gaps, not design decisions
- After EPIC-002, update ADR-003 (LLM Provider Traits) to reflect actual file layout after H-04

---

*Ready for validator review → `/architecture-validator`, `/security-validator`, `/operations-validator`*
