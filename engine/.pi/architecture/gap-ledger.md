# Gap Ledger

> **Source:** Comprehensive codebase assessment — 2026-06-15
> **Last Updated:** 2026-08-28 (Addendum: approval/identity pre-flight findings — 8 new, see bottom)
> **Scope:** All 17 modules across engine/src/, architecture docs, tests, CI, tooling
> **Total findings:** 35 | **Resolved:** 27 | **Open:** 8

---

## Severity Tiers

| Tier | Label | Threshold | Open | Resolved |
|------|-------|-----------|------|----------|
| C | Critical | Must resolve before production use | 0 | 3 |
| H | High | Should resolve in current phase | 2 | 6 |
| M | Medium | Quality improvements, next 2 sprints | 4 | 11 |
| L | Low | Nice to have, backlog | 2 | 7 |

> Open counts include Addendum 2026-08-28 (below).

---

## Critical (C) — Must Resolve Before Production

| ID | Category | Finding | Recommended Action | Effort | Status |
|----|----------|---------|---------------------|--------|--------|
| C-01 | Observability | **Tracing fully instrumented.** `tracing` deps added. `observability/` module with `TracingConfig`, `SpanPrivacy`. 179 `#[tracing::instrument]` annotations across all service methods in 17 modules. | — | — | ✅ **Resolved** |
| C-02 | Observability | **HealthService + MetricsRegistry implemented.** `HealthService` with `HealthCheck` trait, timeout support, aggregation. `MetricsRegistry` with 8 standard metrics (counters, gauges, histograms). `register_all_module_checks()` for 16 modules. | — | — | ✅ **Resolved** |
| C-03 | Testing | **Concurrent-safety tests complete across all 5 modules.** `budget_tracking` (2 tests), `event_system` (2 tests), `dag_engine` (stub), `execution_engine` (concurrency_tests.rs), `state_persistence` (concurrency_tests.rs). All committed. | — | — | ✅ **Resolved** |

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
| M-10 | Performance | **Benchmarks added.** `benches/dag_engine.rs` with criterion benchmarks for topological sort, seal, ready queue, 100-node DAG execution. | — | — | ✅ **Resolved** |
| M-11 | Testing | **Failure-injection tests added.** Circuit breaker (2 tests) + timeout (1 test) in audit/failure_tests.rs. | — | — | ✅ **Resolved** |

## Low (L) — Nice to Have, Backlog

| ID | Category | Finding | Recommended Action | Effort | Status |
|----|----------|---------|---------------------|--------|--------|
| L-01 | Code Quality | **LlmBudgetImpl methods made pub(crate).** Internal state no longer part of public API. | — | — | ✅ **Resolved** |
| L-02 | Code Quality | **Warning dedup documented as intentional.** One-shot design noted with doc comment. | — | — | ✅ **Resolved** |
| L-03 | Code Quality | **commit(&mut self) → commit(&self).** Atomics make mutable borrow unnecessary. | — | — | ✅ **Resolved** |
| L-04 | Testing | **ClaudeTemplateGenerator unit tests added.** 6 tests: strip_code_fences variants, parse_api_response (valid/invalid/missing), build_system_prompt, build_user_message. | — | — | ✅ **Resolved** |
| L-05 | Documentation | **All 17 module docs updated.** Status footers with "Implemented" and "Last verified: 2026-06-15" added. Observability module doc created. | — | — | ✅ **Resolved** |
| L-06 | Tooling | **Coverage installer added.** `install_coverage_tools.sh` for CI. Pre-commit hook for fmt+clippy. | Install `cargo-llvm-cov` in CI. | — | ✅ **Resolved** |
| L-07 | Architecture | **RAII Drop underflow fixed.** `saturating_sub` via `fetch_update` in `LlmBudgetReservationImpl::drop()`. | — | — | ✅ **Resolved** |

---

## Summary Statistics

| Dimension | Critical | High | Medium | Low | Total |
|-----------|----------|------|--------|-----|-------|
| Observability | 0 | 0 | 0 | 0 | 0 |
| Testing | 0 | 0 | 0 | 0 | 0 |
| Architecture | 0 | 0 | 0 | 0 | 0 |
| Code Quality | 0 | 0 | 0 | 0 | 0 |
| Tooling | 0 | 0 | 0 | 0 | 0 |
| Documentation | 0 | 0 | 0 | 0 | 0 |
| Performance | 0 | 0 | 0 | 0 | 0 |
| **Total** | **0** | **0** | **0** | **0** | **0** |

---

*Generated: 2026-06-15 | Updated post-implementation: all 3 epics + 6 batches merged*

---

# Addendum 2026-08-28 — Approval/Identity Epic Pre-flight Findings

> **Source:** Code audit during the approval-binding + identity contract freeze (2026-08-28)
> **Scope:** execution_engine, state_persistence, audit, orchestrator, mcp execution_tools
> **Method:** Verified against actual source (service_impl.rs, orchestrator_impl.rs, state.rs, engine_facade_impl.rs)
> **Total:** 8 findings | **Resolved:** 0 | **Open:** 8
> **Disposition:** No blockers for starting the identity → approval → auth epics. Items H-07/H-08 and M-12..M-14 must be added as issues to the approval epic; M-15 and L-08/L-09 are follow-ups.

## High (H) — Should resolve before the approval feature ships

| ID | Category | Finding | Recommended Action | Address In | Effort | Status |
|----|----------|---------|---------------------|-----------|--------|--------|
| H-07 | Security | **`approve_node` has no `requires_approval` gate.** `ApproveNodeInput` accepts any step name; non-gated nodes can be approved by name (`execution_engine/application/service_impl.rs` → `approve_node`). | Reject approvals for nodes where `requires_approval == false` or the node is not in `AwaitingApproval`; return a structured denial. | Approval epic — R1 issue | S (0.5d) | ⬜ Open |
| H-08 | Observability | **`hydrate_execution` resets `started_at` to `Utc::now()`.** Resumed (approval-paused, GAP-3) runs report a distorted `duration_ms` in the audit envelope. `ExecutionState` already persists `started_at` — it is simply not used on hydrate. | On hydrate, use the persisted `started_at` instead of `Utc::now()` (one-line fix; add a resume-duration regression test). | Approval epic — durability issue | S (0.5d) | ⬜ Open |

## Medium (M) — Address within the approval epic or next two sprints

| ID | Category | Finding | Recommended Action | Address In | Effort | Status |
|----|----------|---------|---------------------|-----------|--------|--------|
| M-12 | Security | **Envelope signing is config-gated.** Production signing is intact (the `signature: None` instances are test helpers only — verified), but a run can complete without an HMAC signature. The approval-evidence story (ADR-011/012) depends on signing. | Require signing for approval-bearing runs (fail or explicitly mark `signature: none` + `evidence: degraded` in the envelope); document the toggle. | Approval epic — evidence issue | S (1d) | ⬜ Open |
| M-13 | Documentation | **`planning_prompt_content: None` TODO.** Prompt capture is documented but not wired (`orchestrator_impl.rs` lines 870, 1385). R4 `decision_context` follows the same privacy pattern. | Wire prompt capture (config-gated, `planning_prompt` pattern) or reuse it for `decision_context` full-payload storage. | Approval epic — R4 issue | M (1-2d) | ⬜ Open |
| M-14 | Observability | **Event-publish failures are swallowed** (`let _ = event_bus.publish(...)`). Deliberate for observability, but a swallowed `ApprovalRecorded` / `IntentMismatchDetected` means approval evidence silently missing from the envelope. | For approval/evidence events: warn-log on publish failure; consider an explicit `evidence: incomplete` marker in the envelope. | Approval epic — evidence issue | S (1d) | ⬜ Open |
| M-15 | Architecture | **`ExecutionState` persists node state twice with two vocabularies** — coarse `node_states` (Pending/InProgress/...) AND `exec_node_states` (runtime Ready/Running/AwaitingApproval, the one resume actually uses). Works; every state-format change doubles. | Consolidate to one persisted representation when adding `approval_records` to the state format (or document the redundancy as intentional). | Approval epic — durability issue / follow-up | M (2-3d) | ⬜ Open |

## Low (L) — Backlog

| ID | Category | Finding | Recommended Action | Address In | Effort | Status |
|----|----------|---------|---------------------|-----------|--------|--------|
| L-08 | Code Quality | **1617 unwrap/expect/panic in non-test code** (concentrated: tools 345, event_system 117, state_persistence 103, enforcement 86, risk_gating 77). CI "zero warnings" covers compiler warnings, not unwraps. | Opportunistic hygiene pass in the files the approval epic touches (execution_engine, audit, state_persistence); replace with typed errors where on the dispatch/evidence path. | Follow-up | M (2-3d) | ⬜ Open |
| L-09 | Observability | **Verify `git_commit`/`git_branch` are actually populated** in the audit envelope. The R5 git-diff effect oracle builds on them as the dispatch-time baseline. | Confirm population at execution start; add a test if missing. | Verify before R5 work | S (0.5d) | ⬜ Open |

## Addendum Summary

| Tier | Open | Resolved |
|------|------|----------|
| C | 0 | 3 |
| H | 2 | 6 |
| M | 4 | 11 |
| L | 2 | 7 |
| **Total** | **8** | **27** |

---

*Addendum generated: 2026-08-28 | Disposition: fold H-07/H-08 + M-12/M-13/M-14 into the approval epic issues when generated; M-15 + L-08/L-09 as follow-ups*
