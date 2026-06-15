# Gap Ledger

> **Source:** Comprehensive codebase assessment — 2026-06-15
> **Scope:** All 17 modules across engine/src/, architecture docs, tests, CI, tooling
> **Total findings:** 27 gaps across 4 severity tiers

---

## Severity Tiers

| Tier | Label | Threshold | Count |
|------|-------|-----------|-------|
| C | Critical | Must resolve before production use | 3 |
| H | High | Should resolve in current phase | 6 |
| M | Medium | Quality improvements, next 2 sprints | 11 |
| L | Low | Nice to have, backlog | 7 |

---

## Critical (C) — Must Resolve Before Production

| ID | Category | Finding | Location | Evidence | Recommended Action | Effort |
|----|----------|---------|----------|----------|---------------------|--------|
| C-01 | Observability | **No logging or tracing infrastructure.** Zero dependency on `tracing`, `log`, or `slog`. System manages LLM calls, retries, cancellations, concurrent DAG execution with no structured diagnostic output. | `engine/Cargo.toml` — no log/tracing deps | Grep for `tracing`, `log`, `slog` across entire repo returns zero results | Add `tracing` crate. Instrument every service method with `#[tracing::instrument]`. Add spans for: LLM API calls, retry decisions, DAG node transitions, budget reserve/commit, cancellation signal propagation. Emit events to tracing as well as EventBus. | L (3-5 days) |
| C-02 | Observability | **No centralized health service and no /metrics endpoints.** Health endpoints exist in 3 modules (`dag_engine`, `state_persistence`, `execution_engine`) with `HEALTH_PATH` constants and `HealthResponse` structs, but 13 other modules lack them. No `/metrics` endpoints exist anywhere for exposing budget consumption, retry rates, circuit-breaker state, or execution throughput. | `interfaces/http/mod.rs` in dag_engine, state_persistence, execution_engine define health; all other modules lack it | `HEALTH_PATH` found in 3/16 modules; `metrics` found in zero files | (1) Add health endpoints to the remaining 13 modules. (2) Create a centralized `HealthService` aggregating: budget status, circuit-breaker states, active executions, event bus stats. (3) Add `/metrics` endpoints with `prometheus` crate for operational visibility (budget consumption rate, retry frequency, execution latency distribution). | M (1-2 weeks) |
| C-03 | Testing | **No concurrent-safety tests.** System uses `RwLock`, `Mutex`, `AtomicU32`, tokio broadcast channels, and `CancellationToken` extensively, but has zero tests exercising concurrent access patterns (e.g., parallel reservations, simultaneous graph mutations, race on pause+abort). | `budget_tracking`, `dag_engine`, `execution_engine`, `state_persistence`, `event_system` | Grep for `tokio::spawn`, `JoinSet`, `try_join` in test files returns zero results | Add `tokio::try_join!` and `tokio::spawn` based concurrency tests for: (a) parallel budget reservations from 10 tasks, (b) simultaneous DAG node completions, (c) pause/resume/abort races, (d) event bus publish+subscribe under load. Use `loom` for lock-free correctness if feasible. | M (1-2 weeks) |

---

## High (H) — Should Resolve in Current Phase

| ID | Category | Finding | Location | Evidence | Recommended Action | Effort |
|----|----------|---------|----------|----------|---------------------|--------|
| H-01 | Code Quality | **24 compiler warnings** — mostly dead code, unused imports, unused variables. Obscures real API surface and may hide genuinely unused unsafe or broken code. | Project-wide | `cargo build` outputs 24 warnings; `cargo fix` can auto-resolve 15 | Run `cargo fix --lib -p rigorix`. Manually review remaining ~9 warnings. Remove dead code or add `#[allow(dead_code)]` with justification comments if reserved for future phases. Add `#![deny(warnings)]` to CI pipeline. | S (1 day) |
| H-02 | Testing | **No cross-module integration test suite.** All tests are `#[cfg(test)]` inline or module-level `tests.rs`. No `tests/` directory exists. Critical paths like "plan → execute → audit" are never tested end-to-end. | `engine/` — no `tests/` directory | `find engine -name "tests" -type d` returns only `src/*/tests.rs` files, no top-level integration tests | Create `engine/tests/` with: (a) `plan_to_execute_integration.rs` — full pipeline from intent to execution, (b) `budget_enforcement_integration.rs` — budget exhaustion triggers cancellation, (c) `audit_trail_integration.rs` — full execution produces complete audit envelope. Use real implementations wired together (no mocks). | M (1-2 weeks) |
| H-03 | Testing | **No live LLM API integration tests.** All classifiers and generators tested with mocks only. `ClaudeClassifier::classify_with_alternatives()` and `ClaudeTemplateGenerator::generate()` never tested against real APIs. | `planning/domain/claude_classifier.rs`, `openai_classifier.rs`, `template_generation/` | All test files use `MockClassifier`, `MockGenerator` — no `#[cfg(feature = "integration")]` tests with real APIs | Add `#[cfg(feature = "live-tests")]` integration tests behind a feature flag. Test: classification accuracy, template generation quality, error handling for API failures, timeout behavior, token usage counting accuracy. Add to CI as optional manual stage. | M (1-2 weeks) |
| H-04 | Architecture | **`planning/domain/` contains concrete LLM implementations.** `ClaudeClassifier`, `OpenAIClassifier`, `MockClassifier`, `MockParameterExtractor` live in `domain/` alongside trait definitions. Clean Architecture says domain should contain only interfaces and entities. | `planning/domain/claude_classifier.rs`, `openai_classifier.rs`, `mock_classifier.rs`, `mock_extractor.rs` | `planning/domain/mod.rs` re-exports `pub mod claude_classifier;` (line ~7) | Move `ClaudeClassifier` → `planning/infrastructure/claude_classifier.rs`. Move `OpenAIClassifier` → `planning/infrastructure/openai_classifier.rs`. Move mocks to `planning/application/` or keep under `#[cfg(test)]`. Update imports across codebase and tests. Run full test suite after move. | M (3-5 days) |
| H-05 | Architecture | **`execution` module is a stub overlapping with `execution_engine`.** Contains only `ExecutionError` — the actual execution logic lives in `execution_engine`. This creates confusion about which module is authoritative. | `execution/` vs `execution_engine/` | `execution/mod.rs` declares `pub mod domain;` only; `execution_engine/` has full 4-layer architecture | Option A (preferred): Merge `execution::ExecutionError` into `execution_engine::domain::ExecutionError`, remove the `execution` module, update all imports. Option B: Add `#[deprecated = "Use execution_engine instead"]` on the `execution` module and remove in next major version. | S-M (1-3 days) |
| H-06 | Code Quality | **`is_retriable()` logic too conservative.** Only I/O errors and HTTP 5xx are considered retriable. Domain errors like `CancellationError`, `LlmBudgetError` could succeed on retry but are hard-classified as non-retriable. | `engine/src/error.rs:is_retriable()` | Method body checks only `Io` and `Http` variants | Add `is_retriable()` to each domain error trait. Let modules self-declare which variants are retriable. `CoreOrchestratorError::is_retriable()` delegates to the inner error's method. | S (1-2 days) |

---

## Medium (M) — Quality Improvements, Next 2 Sprints

| ID | Category | Finding | Location | Evidence | Recommended Action | Effort |
|----|----------|---------|----------|----------|---------------------|--------|
| M-01 | Testing | **Uneven test distribution.** 5 modules have dedicated test files with 374+ tests total; 12 modules have only inline `#[cfg(test)]` blocks. Safety-critical modules like `cancellation` and `enforcement` lack dedicated test files. | Distribution: planning (99), tools (~81), risk_gating (~75), event_system (~63), dag_engine (67), others (scattered) | `enforcement/` has no `tests.rs`; `cancellation/` has no `tests.rs`; `state_persistence/` has no `tests.rs` | Add dedicated `tests.rs` for: enforcement (test preset validation, limit enforcement, tool override precedence), cancellation (test graceful vs immediate shutdown, timeout, child tokens, cleanup handlers), state_persistence (test atomic write-rename, crash recovery, concurrent reads, corrupted state handling). | M (1-2 weeks) |
| M-02 | Testing | **No property-based or fuzz testing.** Serialization roundtrips, DAG operations, budget arithmetic, and hash determinism would benefit from property-based testing. | Project-wide | `Cargo.toml` has no `proptest`, `quickcheck`, or `arbitrary` dependencies | Add `proptest` to dev-dependencies. Write property tests for: (a) `TaskGraph` serde roundtrip — any valid graph survives serialize→deserialize, (b) budget arithmetic — `reserve(N) + commit(M)` for any N>0, M>0 leaves counters consistent, (c) `planning_hash` — same inputs always produce same hash, different inputs never collide. | M (1-2 weeks) |
| M-03 | Tooling | **No supply chain security.** No `cargo-audit` or `cargo-deny` integration. Dependencies include `tree-sitter`, `reqwest`, `tokio` — all with known historical CVEs. | `engine/Cargo.toml` | No `.cargo/deny.toml`, no `cargo audit` in CI scripts | Add `cargo-deny` with configuration banning: unlicensed crates, known CVEs (with advisory DB), wildcard dependencies. Add `cargo audit` to CI as a required check. Run both on every PR. | S (1 day) |
| M-04 | Tooling | **No pre-commit hook for linting; CI linting exists but may not gate merges.** `cargo clippy` and `cargo fmt --check` exist in `stage_lint.sh` and `run_preflight.sh`, but there is no pre-commit hook to catch issues before push, and the CI scripts log failures via `log_fail` without clear evidence that lint failures block PRs. | `.pi/scripts/ci/stage_lint.sh:92-97`, `.pi/scripts/ci/run_preflight.sh:270-273`, no pre-commit hook | CI has lint commands; no `pre-commit` hook exists (only `.sample`) | (1) Add a pre-commit hook running `cargo fmt --check && cargo clippy -- -D warnings` on staged files. (2) Verify CI lint stages exit with non-zero on failure and gate PR merges. (3) Consider `clippy::pedantic` for new code. | S (1 day) |
| M-05 | Code Quality | **`failure_classification/classify.rs` is a top-level file breaking the 4-layer pattern.** All other modules have only layer directories at their root. This file is a legacy artifact. | `failure_classification/classify.rs` | `failure_classification/mod.rs` declares `pub mod classify;` alongside `pub mod domain; pub mod application;` etc. | Move `classify_failure()` free function into `failure_classification/application/` as part of the classifier service, or into `domain/` if it's pure domain logic. Remove the top-level `classify.rs` file. | XS (30 min) |
| M-06 | Code Quality | **No shared `Result` type alias.** Every fallible function spells out `Result<T, CoreOrchestratorError>` verbosely. Changing the error type would require updating 87 signatures across 15 files. | Project-wide — 15 files reference `CoreOrchestratorError` in return types | `CoreOrchestratorError` appears 87 times across 15 files (including `error.rs` with 73 self-references) | Add `pub type Result<T> = std::result::Result<T, CoreOrchestratorError>;` to `lib.rs`. Update function signatures project-wide. This is mechanical and safe. | S (1 day, mostly automated) |
| M-07 | Code Quality | **DTO validation patterns duplicated across modules.** Multiple modules define similar `ValidationError`, `valid()`, `errors()`, `warnings()` patterns independently. | `configuration/application/dto/`, `cancellation/application/dto/`, others | Grep for `ValidationError` across modules — each module defines its own | Extract a shared `ValidationResult<T>` type with `valid()`, `errors()`, `warnings()` methods. Place in a new `engine/src/common/` module or keep as a lightweight shared utility. Update all DTOs to use it. | S (1-2 days) |
| M-08 | Architecture | **`template_generation` module is sparse.** Recently extracted from `planning` (#205, #206). Has domain/, application/, infrastructure/, interfaces/ directories but `application/` lacks `*_impl.rs` files — only trait stubs. | `template_generation/application/` | `application/` contains `factory.rs`, `service.rs`, `dto/`, `mod.rs` — no `*_impl.rs` files | Complete the `template_generation` module with: (a) `TemplateGenerationServiceImpl` — orchestrates generator calls with retry, (b) factory implementations, (c) infrastructure/repository concrete implementations. Add dedicated tests. Align with Phase 3 completion criteria from implementation roadmap. | L (1-2 weeks) |
| M-09 | Documentation | **ADR statuses not updated since initial scaffold.** 8 ADRs created on 2026-06-13 with status "Accepted" but no subsequent updates reflecting implementation decisions made during Phase 1-3 work. | `engine/.pi/architecture/decisions/ADR-*.md` | CHANGELOG shows significant implementation since ADR creation but ADRs not updated | Review each ADR against current implementation. Update status to "Implemented" where applicable. Add "Superseded by" notes if implementation diverged. Add new ADRs for decisions made during implementation (e.g., choice to co-locate impls with traits in application layer). | S (1 day) |
| M-10 | Performance | **DAG execution performance with 100+ nodes is a documented risk with no benchmarks.** The implementation roadmap lists this as a risk but no benchmark harness exists. | `engine/.pi/architecture/implementation-roadmap.md` — Risk Assessment table | No `benches/` directory, no `criterion` dependency, no `[[bench]]` entries in Cargo.toml | Add `criterion` to dev-dependencies. Create benchmarks for: (a) `topological_sort()` with 10/50/100/500 nodes, (b) `seal_graph()` with varying node counts, (c) `get_ready_nodes()` performance under load, (d) full execution of a 100-node DAG. Run in CI as informational (non-blocking). | M (3-5 days) |
| M-11 | Testing | **No failure-injection testing.** System handles network failures, LLM API timeouts, filesystem errors — but none of these scenarios are tested. | `audit/`, `planning/`, `execution_engine/` | Test files test happy paths and domain error construction — no simulated IO failures | Add tests using `tokio::time::timeout` to simulate: (a) LLM API timeout during classification/generation, (b) filesystem write failure during state persistence, (c) network error during audit send, (d) partial writes. Verify system degrades gracefully (retry, circuit breaker, rollback). | M (1-2 weeks) |

---

## Low (L) — Nice to Have, Backlog

| ID | Category | Finding | Location | Evidence | Recommended Action | Effort |
|----|----------|---------|----------|----------|---------------------|--------|
| L-01 | Code Quality | **`LlmBudgetImpl` exposes internal state via public methods** (`max_calls()`, `calls_used()`, `remaining_calls()`, `cancel_token()`). These bypass the `LlmBudgetService` trait and create a second API surface. The trait already provides `get_status()` and `has_capacity()`. | `budget_tracking/application/llm_budget_impl.rs:86-113` | Lines 86-113 define 6 public methods on `LlmBudgetImpl` that are not part of the `LlmBudgetService` trait | Make these methods `pub(crate)` or remove them. Consumers should use the trait interface. The `cancel_token()` accessor is needed by the orchestrator — consider adding `cancellation_token()` to the `LlmBudgetService` trait instead. | XS (30 min) |
| L-02 | Code Quality | **`check_call_warning()` and `check_token_warning()` have a logic bug.** After the first call, both methods return `None` for all subsequent calls because `*_warning_emitted` is never reset. Warning state is permanently consumed. | `budget_tracking/application/llm_budget_impl.rs:116-159` | Lines 118-119 and 141-142: `if self.state.*_warning_emitted.load(...) { return None; }` — once set, never cleared | Evaluate the intended behavior: (a) if warnings should fire once per budget lifetime, document this explicitly, (b) if warnings should fire at each threshold crossing (80%, 90%, 95%), implement multi-level thresholds with separate flags, (c) add a `reset_warnings()` method for testing. | XS-S (30 min - 2 hours) |
| L-03 | Code Quality | **`LlmBudgetReservationImpl::commit()` uses `&mut self` for logical mutation of atomics.** The method takes `&mut self` — meaning only one caller can hold a mutable reference — but all state is behind `Arc<Atomic*>`, so `&self` would suffice and enable shared access. | `budget_tracking/application/llm_budget_impl.rs:382` | Line 382: `async fn commit(&mut self, ...)` — mutable borrow unnecessary since all fields are atomics | Change signature to `&self`. The `AtomicBool` fields (`committed`, `rolled_back`) provide internal synchronization. This enables sharing the reservation guard via `Arc`. | XS (15 min) |
| L-04 | Testing | **`ClaudeTemplateGenerator::generate()` and `ClaudeClassifier::classify_with_alternatives()` untested.** Only utility methods (`parse_response`, `strip_code_fences`, config defaults) are tested. The actual API-calling methods have zero test coverage. | `planning/domain/claude_classifier.rs`, `template_generation/domain/generator.rs` | Test files test `parse_response` with mock JSON strings but never call the API methods | Add unit tests for: request building (correct model, headers, body format), response parsing (success, error, malformed), timeout handling. These can test the HTTP interaction layer without live APIs by providing mock reqwest clients. | S (1-2 days) |
| L-05 | Documentation | **Module architecture docs describe blueprints, not current state.** Some `.pi/architecture/modules/*.md` files were created during initial scaffold and may not reflect implementation changes (e.g., `template-generation.md` pre-dates its extraction from planning). | `engine/.pi/architecture/modules/` | CHANGELOG shows significant implementation; module docs may be stale | Audit each module doc against actual code. Update to reflect: implemented traits, concrete types, file locations, test coverage. Add "Last verified against code" timestamp to each doc. | M (2-3 days) |
| L-06 | Tooling | **Coverage scripts rely primarily on test-function counting, not line/branch measurement.** Scripts attempt `cargo-tarpaulin` and `cargo-llvm-cov` when available, but fall back to `grep -c "#\[test\]"` counting when no coverage tool is installed. There is no guarantee actual coverage tooling is present in CI. | `engine/.pi/scripts/ci/check_*_coverage.sh` | Coverage scripts use counting as the primary verification method with tool-based coverage as optional enhancement | Ensure `cargo-llvm-cov` or `tarpaulin` is installed in CI environment. Promote tool-based coverage as the primary check; keep test counting as a fast pre-check. Set coverage thresholds per module (e.g., 80% line coverage). Generate HTML reports. Make coverage a non-blocking informational check initially, then promote to required. | S-M (1-3 days) |
| L-07 | Architecture | **ADR-008 (RAII Budget Reservation) implementation has a subtle issue.** The `Drop` impl for `LlmBudgetReservationImpl` performs `fetch_sub` but does not check for underflow. If a bug causes double-rollback, counters could go negative (wrapping in release mode). | `budget_tracking/application/llm_budget_impl.rs:364-377` | Lines 370-375: `fetch_sub(1, ...)` and `fetch_sub(self.reserved_tokens, ...)` without underflow checks | Add `saturating_sub` logic or use `compare_exchange` loop to prevent underflow. Alternatively, add `debug_assert!` that counters >= subtraction amount (catches bugs in debug/test but not release). | XS (15 min) |

---

## Summary Statistics

| Dimension | Critical | High | Medium | Low | Total |
|-----------|----------|------|--------|-----|-------|
| Observability | 2 | 0 | 0 | 0 | 2 |
| Testing | 1 | 2 | 3 | 1 | 7 |
| Architecture | 0 | 2 | 1 | 1 | 4 |
| Code Quality | 0 | 1 | 3 | 3 | 7 |
| Tooling | 0 | 0 | 2 | 1 | 3 |
| Documentation | 0 | 0 | 1 | 1 | 2 |
| Performance | 0 | 0 | 1 | 0 | 1 |
| **Total** | **3** | **6** | **11** | **7** | **27** |

## Effort Estimates

| Effort | Label | Count | Cumulative |
|--------|-------|-------|------------|
| XS (< 1 hour) | Quick fix | 5 | 5 |
| S (1-2 days) | Small | 9 | 14 |
| M (3 days - 2 weeks) | Medium | 11 | 25 |
| L (2+ weeks) | Large | 2 | 28 |

## Suggested Triage Order (First 2 Weeks)

```
Week 1:
  Day 1:   C-01 (tracing setup) — start
  Day 2:   H-01 (fix 24 warnings) + M-05 (remove classify.rs) + M-06 (Result alias)
  Day 3:   M-09 (update ADRs)
  Day 4:   H-04 (move classifiers out of domain) — start
  Day 5:   H-04 (finish) + H-06 (is_retriable per-module)

Week 2:
  Day 1-2: H-05 (merge execution → execution_engine) + C-01 (complete)
  Day 3-5: M-01 (add tests to enforcement, cancellation, state_persistence)
  Day 3-5: M-03 (cargo-deny + cargo-audit) + M-04 (rustfmt + clippy in CI)
```

---

*Generated: 2026-06-15 | Based on comprehensive codebase assessment (296 files, ~56K lines, 17 modules)*
