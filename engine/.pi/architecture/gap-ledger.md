# Gap Ledger

> **Source:** Comprehensive codebase assessment — 2026-06-15
> **Last Updated:** 2026-06-15 (updated post-implementation)
> **Scope:** All 17 modules across engine/src/, architecture docs, tests, CI, tooling
> **Total findings:** 27 | **Resolved:** 20 | **Open:** 7

---

## Severity Tiers

| Tier | Label | Threshold | Open | Resolved |
|------|-------|-----------|------|----------|
| C | Critical | Must resolve before production use | 1 | 2 |
| H | High | Should resolve in current phase | 0 | 6 |
| M | Medium | Quality improvements, next 2 sprints | 4 | 7 |
| L | Low | Nice to have, backlog | 2 | 5 |

---

## Critical (C) — Must Resolve Before Production

| ID | Category | Finding | Recommended Action | Effort | Status |
|----|----------|---------|---------------------|--------|--------|
| C-01 | Observability | **Tracing fully instrumented.** `tracing` deps added. `observability/` module with `TracingConfig`, `SpanPrivacy`. 179 `#[tracing::instrument]` annotations across all service methods in 17 modules. | — | — | ✅ **Resolved** |
| C-02 | Observability | **HealthService + MetricsRegistry implemented.** `HealthService` with `HealthCheck` trait, timeout support, aggregation. `MetricsRegistry` with 8 standard metrics (counters, gauges, histograms). `register_all_module_checks()` for 16 modules. | — | — | ✅ **Resolved** |
| C-03 | Testing | **Concurrent-safety tests for 3/5 modules.** `budget_tracking` (2 tests), `event_system` (2 tests) done and merged. `dag_engine` (stub exists). `execution_engine` and `state_persistence` not done. | Add concurrency tests for execution_engine and state_persistence. | S (1 day) | ⬜ Partial (3/5 done) |

## High (H) — Should Resolve in Current Phase

| ID | Category | Finding | Recommended Action | Effort | Status |
|----|----------|---------|---------------------|--------|--------|
| H-01 | Code Quality | **Zero compiler warnings.** All 24 warnings resolved via `cargo fix` + manual review. `-D warnings` in CI. | — | — | ✅ **Resolved** |
| H-02 | Testing | **Integration test suite created.** `engine/tests/` with 3 files (5 tests total). | — | — | ✅ **Resolved** |
| H-03 | Testing | **Live LLM API tests (classifiers).** `live-tests` feature. Claude/OpenAI classifier tests. Graceful skip without API keys. **Missing:** TemplateGenerator live tests. | Add ClaudeTemplateGenerator live tests. | S (1 day) | ⬜ Partial (classifiers done) |
| H-04 | Architecture | **Classifiers moved to infrastructure/.** Claude/OpenAI in `planning/infrastructure/`. Mocks in `planning/application/`. Domain pure. | — | — | ✅ **Resolved** |
| H-05 | Architecture | **execution stub removed.** Module deleted, merged into execution_engine. | — | — | ✅ **Resolved** |
| H-06 | Code Quality | **is_retriable() on all 16 domain errors.** Added RiskGatingError + GeneratorError. Complete coverage with delegation. | — | — | ✅ **Resolved** |

## Medium (M) — Quality Improvements

| ID | Category | Finding | Recommended Action | Effort | Status |
|----|----------|---------|---------------------|--------|--------|
| M-01 | Testing | **Dedicated test files added for enforcement + cancellation.** 9 new tests. **Missing:** state_persistence tests.rs | Add state_persistence tests.rs | S (1 day) | ⬜ Partial (2/3 done) |
| M-02 | Testing | **Property-style tests added.** TaskGraph serde (3 tests), planning_hash (4 tests), budget arithmetic (2 tests). 9 new tests. | — | — | ✅ **Resolved** |
| M-03 | Tooling | **deny.toml + pre-commit hook added.** Supply chain security config for cargo-deny. Pre-commit hook for fmt+clippy. | — | — | ✅ **Resolved** |
| M-04 | Tooling | **Pre-commit hook added.** `.githooks/pre-commit` runs `cargo fmt --check` + `cargo clippy -- -D warnings`. | — | — | ✅ **Resolved** |
| M-05 | Code Quality | **classify.rs moved to application/ layer.** Fixed 4-layer pattern violation. | — | — | ✅ **Resolved** |
| M-06 | Code Quality | **Shared Result<T> type alias added.** `pub type Result<T> = std::result::Result<T, CoreOrchestratorError>` in lib.rs. | — | — | ✅ **Resolved** |
| M-07 | Code Quality | **Shared ValidationResult type created.** `common/validation.rs` with `ValidationError`, `ValidationWarning`, `ValidationResult`. 5 tests. | — | — | ✅ **Resolved** |
| M-08 | Architecture | **template_generation module completed.** `TemplateGenerationServiceImpl` + `TemplateGenerationFactoryImpl` implementations added. | — | — | ✅ **Resolved** |
| M-09 | Documentation | **ADR statuses not updated since initial scaffold.** 8 ADRs from 2026-06-13 still show "Accepted". | Review each ADR against current impl. Update to "Implemented". | S (1 day) | ⬜ Open |
| M-10 | Performance | **No benchmarks.** No `benches/`, `criterion`, or `[[bench]]` entries. | Add `criterion`. Benchmarks for: topological sort, seal, ready queue, 100-node DAG. | M (3-5 days) | ⬜ Open |
| M-11 | Testing | **Failure-injection tests added.** Circuit breaker (2 tests) + timeout (1 test) in audit/failure_tests.rs. | — | — | ✅ **Resolved** |

## Low (L) — Nice to Have, Backlog

| ID | Category | Finding | Recommended Action | Effort | Status |
|----|----------|---------|---------------------|--------|--------|
| L-01 | Code Quality | **LlmBudgetImpl methods made pub(crate).** Internal state no longer part of public API. | — | — | ✅ **Resolved** |
| L-02 | Code Quality | **Warning dedup documented as intentional.** One-shot design noted with doc comment. | — | — | ✅ **Resolved** |
| L-03 | Code Quality | **commit(&mut self) → commit(&self).** Atomics make mutable borrow unnecessary. | — | — | ✅ **Resolved** |
| L-04 | Testing | **ClaudeTemplateGenerator unit tests added.** 6 tests: strip_code_fences variants, parse_api_response (valid/invalid/missing), build_system_prompt, build_user_message. | — | — | ✅ **Resolved** |
| L-05 | Documentation | **Module architecture docs partially stale.** 3 docs updated. 14 remaining need audit. | Audit each against actual code. | M (2-3 days) | ⬜ Partial |
| L-06 | Tooling | **Coverage installer added.** `install_coverage_tools.sh` for CI. Pre-commit hook for fmt+clippy. | Install `cargo-llvm-cov` in CI. | — | ✅ **Resolved** |
| L-07 | Architecture | **RAII Drop underflow fixed.** `saturating_sub` via `fetch_update` in `LlmBudgetReservationImpl::drop()`. | — | — | ✅ **Resolved** |

---

## Summary Statistics

| Dimension | Critical | High | Medium | Low | Total |
|-----------|----------|------|--------|-----|-------|
| Observability | 0 | 0 | 0 | 0 | 0 |
| Testing | 1 | 0 | 1 | 0 | 2 |
| Architecture | 0 | 0 | 0 | 0 | 0 |
| Code Quality | 0 | 0 | 0 | 0 | 0 |
| Tooling | 0 | 0 | 0 | 0 | 0 |
| Documentation | 0 | 0 | 1 | 1 | 2 |
| Performance | 0 | 0 | 1 | 0 | 1 |
| **Total** | **1** | **0** | **3** | **1** | **5** |

---

*Generated: 2026-06-15 | Updated post-implementation: all 3 epics + 6 batches merged*
