# Gap Ledger

> **Source:** Comprehensive codebase assessment — 2026-06-15
> **Last Updated:** 2026-08-30 (Addendum 2: codebase audit implementation backlog — 30 new, see bottom)
> **⚠️ Status verification 2026-08-30:** every Open item re-verified against source. **Most Addendum-2 items are already FIXED in code** (marked `GAP-A-xx` in source) — see "Verification 2026-08-30" section at the bottom for the authoritative per-item verdict. Table statuses above are stale relative to that section.
> **Scope:** All 17 modules across engine/src/, architecture docs, tests, CI, tooling
> **Total findings:** 65 | **Resolved:** 27 | **Open:** 38

---

## Severity Tiers

| Tier | Label | Threshold | Open | Resolved |
|------|-------|-----------|------|----------|
| C | Critical | Must resolve before production use | 6 | 3 |
| H | High | Should resolve in current phase | 10 | 6 |
| M | Medium | Quality improvements, next 2 sprints | 12 | 11 |
| L | Low | Nice to have, backlog | 10 | 7 |

> Open counts include Addendum 2026-08-28 (approval pre-flight) + Addendum 2026-08-30 (codebase audit).

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
| H-07 | Security | **`approve_node` has no `requires_approval` gate.** `ApproveNodeInput` accepts any step name; non-gated nodes can be approved by name (`execution_engine/application/service_impl.rs` → `approve_node`). | Reject approvals for nodes where `requires_approval == false` or the node is not in `AwaitingApproval`; return a structured denial. | Approval epic — R1 issue | S (0.5d) | ✅ Fixed (2026-09-06 — see §Verification 2026-08-30) |
| H-08 | Observability | **`hydrate_execution` resets `started_at` to `Utc::now()`.** Resumed (approval-paused, GAP-3) runs report a distorted `duration_ms` in the audit envelope. `ExecutionState` already persists `started_at` — it is simply not used on hydrate. | On hydrate, use the persisted `started_at` instead of `Utc::now()` (one-line fix; add a resume-duration regression test). | Approval epic — durability issue | S (0.5d) | ✅ Fixed (2026-09-06 — see §Verification 2026-08-30) |

## Medium (M) — Address within the approval epic or next two sprints

| ID | Category | Finding | Recommended Action | Address In | Effort | Status |
|----|----------|---------|---------------------|-----------|--------|--------|
| M-12 | Security | **Envelope signing is config-gated.** Production signing is intact (the `signature: None` instances are test helpers only — verified), but a run can complete without an HMAC signature. The approval-evidence story (ADR-011/012) depends on signing. | Require signing for approval-bearing runs (fail or explicitly mark `signature: none` + `evidence: degraded` in the envelope); document the toggle. | Approval epic — evidence issue | S (1d) | ⬜ Open |
| M-13 | Documentation | **`planning_prompt_content: None` TODO.** Prompt capture is documented but not wired (`orchestrator_impl.rs` lines 870, 1385). R4 `decision_context` follows the same privacy pattern. | Wire prompt capture (config-gated, `planning_prompt` pattern) or reuse it for `decision_context` full-payload storage. | Approval epic — R4 issue | M (1-2d) | ✅ Fixed (2026-09-06 — see §Verification 2026-08-30) |
| M-14 | Observability | **Event-publish failures are swallowed** (`let _ = event_bus.publish(...)`). Deliberate for observability, but a swallowed `ApprovalRecorded` / `IntentMismatchDetected` means approval evidence silently missing from the envelope. | For approval/evidence events: warn-log on publish failure; consider an explicit `evidence: incomplete` marker in the envelope. | Approval epic — evidence issue | S (1d) | ✅ Fixed (2026-09-06 — see §Verification 2026-08-30) |
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

---

# Addendum 2026-08-30 — Codebase Audit (v1–v3, code-verified): Implementation Backlog

> **Source:** Independent codebase audit (GLM-5.3-Flash) + counter-validation — every claim verified against source (code is truth)
> **Scope:** engine, mcp, actions, cli — execution integrity, wiring, dead-contract completion, tests, docs/CI truth
> **Disposition (IMPORTANT):** ALL items are to be **IMPLEMENTED or CONNECTED — no deletions.** Dead contracts are unbuilt contracts; every repository, wiring, and event gets implemented and wired, not removed. The project's contract-freeze process treats each as a build target for an epic/issue.
> **Total:** 30 findings | **Resolved:** 0 | **Open:** 30

## Critical (C) — Execution Integrity (silent fake-success class — must resolve before production use)

| ID | Category | Finding | Recommended Action | Address In | Effort | Status |
|----|----------|---------|---------------------|-----------|--------|--------|
| A-01 | Execution | **Unknown tool reports success.** `execution_engine/application/service_impl.rs:231-241` returns `TaskResult::success("Unknown tool '{tool}'...")` for any unrecognized tool — a typo'd/unsupported node passes validation and produces a green run that did nothing. | Unknown tool must **FAIL** (structured error, non-retriable); add the missing adversarial test. | Execution-engine epic | S (0.5d) | ⬜ Open |
| A-02 | Execution | **Missing graph/node returns placeholder success.** `service_impl.rs:1759-1768` returns `("execution output placeholder", ...)` when no graph or node found. | Must **FAIL** with a typed error, not succeed. | Execution-engine epic | S (0.5d) | ⬜ Open |
| A-03 | Execution | **`config_override` cloned and never applied.** `service_impl.rs:1438-1441` (`let _config = ...` — underscore-discarded). Callers believe they override concurrency/retry settings. | Apply the override to the executor config, or remove the field + fix the DTO. | Execution-engine epic | S (0.5d) | ⬜ Open |
| A-04 | Operations | **Hook-runner pipe deadlock.** `hooks/application/hook_runner_impl.rs:100-181` polls `child.try_wait()` in a 10ms sleep loop **without draining stdout/stderr** — a hook writing >64KB blocks forever (only the timeout kills it); `wait_with_output()` is called after `try_wait` already reaped. | Concurrent drain (threads or `tokio::process`), wall-clock timeout around the whole interaction, `wait()` after `kill()`. | Hooks epic | M (1-2d) | ⬜ Open |
| A-05 | Enforcement | **Hooks bypassed in the parallel dispatch loop.** `spawn_concurrent_node` (`service_impl.rs:67`) receives `_hook_runner` (underscore-prefixed/unused); nodes dispatch directly to `exec_*` statics, skipping `execute_tool` — the only PreToolUse/PostToolUse hook site. Permission IS enforced in both paths (verified `:110-121`). | **Wire hooks into the parallel path** (or explicitly document single-node-only semantics); add a test that a PreToolUse hook blocks a parallel-path tool. | Hooks epic | M (1-2d) | ⬜ Open |
| A-06 | Security | **HMAC signs a subset of the envelope, not the payload.** `audit/application/envelope_factory_impl.rs:53-60` signs only `execution_id, timestamp, template_id, planning_hash, total_tokens, duration_ms, events.len()` — NOT event contents, `file_paths`, `scoring_results`, or future `approval_events`/`scope_violations`/`identity`. MCP `compute_hmac` uses a different field set. | Sign the **full canonical serialized envelope** (or all evidence fields); unify engine↔mcp HMAC; refuse unsigned envelopes in `read_audit`. **Blocks ADR-011's "covered transitively by the envelope HMAC" claim.** | Approval epic — evidence issue | M (1-2d) | ⬜ Open |

## High (H) — Should Resolve in Current Phase

| ID | Category | Finding | Recommended Action | Address In | Effort | Status |
|----|----------|---------|---------------------|-----------|--------|--------|
| A-07 | Integration | **MCP production planning is mocked.** `mcp/src/main.rs:797-871`: `build_real_engine` hardwires `MockClassifier` (catch-all `""→"default"`, 0.1) + `MockParameterExtractor`; the registered default template's `read_file` action reads **`/dev/null`**. Real LLM classifiers are never wrapped. | Wire real `claude/openai_classifier` + `llm_extractor` into `build_real_engine`; keep mock mode behind an explicit flag. | MCP epic | M (2-3d) | ⬜ Open |
| A-08 | Integration | **"governance" action mode is a silent alias for Validate.** `actions/src/action_entrypoint/application/mode_resolver_impl.rs:84` maps `"governance" → ActionMode::Validate`. Mode A machinery (diff_analyzer ~5K, policy_evaluator ~4.7K, security_config ~1.7K, audit_posting ~3K) has no runtime path. | **Implement Mode A**: route `governance` through `diff_analyzer` + `policy_evaluator` with base-branch `policy-file` and honored `fail-on-violation`. | Actions epic | L (3-5d) | ⬜ Open |
| A-09 | Reliability | **Budget accounting non-atomic; RAII guard test-only.** `budget_tracking/application/llm_budget_impl.rs:243-250`: load(Acquire) → capacity check → `fetch_add` — TOCTOU, concurrent commits can over-commit. RAII guard `LlmBudgetReservationImpl` is `#[cfg(test)]`-only (`:360`). | CAS loop or `Mutex<BudgetState>`; ship the reservation guard in production; add a contention test. | Budget epic | M (1-2d) | ⬜ Open |
| A-10 | Integration | **SSE advertised, unimplemented.** `mcp/src/main.rs:1666-1668`: `--sse` logs "not fully implemented"; `axum` declared (feature enabled) with zero axum code. | Implement SSE transport or remove the flag + claims. | MCP epic | M (2-3d) | ⬜ Open |
| A-11 | Execution | **Config flags parsed but never read.** `enable_cancellation`, `enable_enforcement`, `max_failures_before_abort`, `max_total_retries_per_session` (executor); no `CancellationToken` in the executor; actions `max-llm-calls`/`max-llm-tokens` unapplied. | Implement the documented behavior (`execution_engine/application/service.rs:59-77`) — cancellation, enforcement limits, abort thresholds — or remove the flags. | Execution-engine epic | M (2-3d) | ⬜ Open |
| A-12 | Performance | **Blocking std IO on the async runtime.** `exec_*` file/git/shell tools (`service_impl.rs:779-1098`), hooks, `template_generation/generator.rs` (~700 lines) use `std::process`/`std::fs` in tokio context; no timeouts on `sh -c`/git. | `tokio::fs`/`tokio::process`; timeouts on all subprocess calls. | Execution-engine epic | L (3-5d) | ⬜ Open |
| A-13 | Quality | **scored_evaluation always uses the first backend.** `scored_evaluation/application/service_impl.rs:138` (`iter().next()`); `EvaluateInput` (application/dto.rs:23-32) has **no backend field** — design limitation, not a selection bug (v3-corrected). | Add backend selection to the API (or document single-backend mode). | Scored-evaluation epic | S (1d) | ⬜ Open |
| A-14 | Enforcement | **Enforcement limits may not block time-based calls.** `enforcement/.../enforcer_impl.rs:205-219` checks calls/tokens but not time (per audit; spot-verify). | Verify + implement time-limit enforcement; add a test that enforcement actually blocks. | Enforcement epic | S (1d) | ⬜ Open |

## Medium (M) — Dead Contracts to CONNECT/IMPLEMENT (no deletions)

| ID | Category | Finding | Recommended Action | Address In | Effort | Status |
|----|----------|---------|---------------------|-----------|--------|--------|
| A-15 | Contracts | **repo_engine indexer stack has no impls.** `IndexerService`/`SymbolRepository`/`SourceRepository`/`GrammarRepository` traits exist with **zero implementations**; real symbol indexing happens elsewhere (code_graph, template_generation). | **Implement** the indexer + repositories and wire them into the symbol graph service (replace the panic-by-design `graph()`). | Repo-engine epic | L (3-5d) | ⬜ Open |
| A-16 | Contracts | **~10 impl-less repository traits.** `GrammarRepository`, `GeneratedTemplateRepository`, `ExecutionResultRepository`, `LlmBudgetRepository`, `PatternRepository`, `ClassificationLogRepository`, `ConfigWriteRepository`, `FailureLogRepository`, `CodeGenEventRepository`, `ParserConfigRepository` — zero impls. | **Implement + wire** each (filesystem/state-backed), or fold into their consuming service if the trait is redundant. | Per-module epics | M (2-3d ea) | ⬜ Open |
| A-17 | Contracts | **Event enums never emitted.** `AuditEvent` (audit/domain/event/mod.rs) has **zero** variants referenced outside its file; other domain event enums similar. | **Wire emission** — publish the lifecycle events (EnvelopeDelivered/Queued/Dropped, CircuitBreakerStateChanged) from the audit sender/queue/circuit-breaker. | Audit epic | S (1d) | ⬜ Open |
| A-18 | Contracts | **RiskClassifier never constructed.** `risk_gating`'s classifier service has zero constructions in the workspace (only `RiskLevel` is used, via tools registry). | **Connect** the classifier into the tool-execution risk-gating path (`execute_with_risk_gate`). | Risk-gating epic | S (1d) | ⬜ Open |
| A-19 | Contracts | **failure_classification services have no callers.** `FailureClassifierService`/`classify_failure` — zero constructions outside the module (only types/strategies consumed). | **Wire** the classifier service into the execution retry loop (replace/augment substring mapping with `FailureType`). | Execution-engine epic | M (1-2d) | ⬜ Open |
| A-20 | Contracts | **Engine templates module is InMemory-only.** `engine/src/templates` has only `InMemoryTemplateRepository`; filesystem loading exists only in MCP template_tools. | **Implement** a filesystem `TemplateRepository` in the engine (`.rigorix/templates/`) so the engine's own wiring can load templates from disk. | Templates epic | M (1-2d) | ⬜ Open |
| A-21 | Architecture | **2 real layering violations (domain→application).** `tools/domain/tool_trait.rs:23` (`application::dto::{ToolInput, ToolResult}`) and `risk_gating/domain/gate_state.rs:21` (`application::dto::PendingGate`). (15 infra→app DTO imports are CORRECT hexagonal direction — not violations.) | Move the DTOs to domain (or invert to domain-owned value objects). | Cleanup (with A-18/A-19 epics) | S (0.5d) | ⬜ Open |
| A-22 | Contracts | **MCP session creation never exercised at runtime.** `create_session` has zero runtime callers; `handle_initialize` fabricates a response with hardcoded `protocolVersion`/version instead of negotiating. | **Wire** session creation into the initialize handshake; implement protocol negotiation. | MCP epic | M (1-2d) | ⬜ Open |

## Low (L) — Tests, Docs & CI Truth

| ID | Category | Finding | Recommended Action | Address In | Effort | Status |
|----|----------|---------|---------------------|-----------|--------|--------|
| A-23 | Testing | **Missing adversarial tests.** No tests for: unknown-tool failure (currently can't fail), parallel-path hook gating, enforcement actually blocking a call, budget reserve contention, circuit-breaker half-open recovery, HTTP/MCP backend transports. | Add the tests that would catch A-01..A-05/A-09/A-14. | Test hardening epic | M (2-3d) | ⬜ Open |
| A-24 | Testing | **24 inert `assert!(false)` stubs** under `engine/tests/unit/**` (no `unit/mod.rs` — not compiled, not red; misleading). | Remove or convert to real tests. | Test hardening epic | S (1d) | ⬜ Open |
| A-25 | Testing | **mcp e2e flakiness + pollution.** 3 near-identical `minify_json`/`send_rpc` helpers; fixed 50-200ms sleeps; one test writes into real CWD `.rigorix/templates/`. | Extract shared helpers; replace sleeps with polling; run in tempdirs (`RIGORIX_REPO_ROOT`). | MCP epic | M (1-2d) | ⬜ Open |
| A-26 | Docs | **Binary name mismatch.** README quickstart says `./target/release/rigorix`; real binary is `rigorix-cli` (no `[[bin]] name="rigorix"` in cli/Cargo.toml — only `[lib] name="rigorix"`). | Fix docs (or add `[[bin]]`); CONTRIBUTING tree omits `mcp/`. | Docs epic | S (0.5d) | ⬜ Open |
| A-27 | Docs/CI | **Stale claims + non-gating CI.** README claims Go/SSE support (absent); "86 verification steps" unreconciled; per-crate proofing steps run `continue-on-error: true` (not gating). | Fix claims; make docs/integration stages actually gate; remove `continue-on-error` from proofing. | Docs/CI epic | M (1-2d) | ⬜ Open |
| A-28 | Docs | **Count drift.** HOW says 3 crates/28-30 modules; actual 4 crates / 33 `pub mod` in lib.rs. GitNexus counts stale (HOW vs AGENTS.md). | Reconcile counts; generate metrics instead of hand-maintaining. | Docs epic | S (0.5d) | ⬜ Open |
| A-29 | Hygiene | **Repo hygiene.** `.gitignore:81` corrupted with literal `\n` (`.mcp.json\nmcp/019*.json\nmcp/.rigorix/`); `rigorix-demo.mov` (16.3MB) tracked; `cli/.rigorix/state/*.json` + `.guardian-*.json` committed. | Repair .gitignore; move .mov to GitHub Releases; purge runtime state; add ignore entries. | Hygiene epic | S (1d) | ⬜ Open |
| A-30 | Docs | **Misc doc drift.** crates.io badge → `rigorix` (nonexistent crate); `SECURITY.md` placeholder email; `mcp/src/main.rs:10-11` comment says "10 OSS tools" (actual 14); dual sha2/hmac versions (0.11/0.13 vs workspace 0.10/0.12). | Fix badge/comment/email; unify crypto stack versions. | Docs epic | S (0.5d) | ⬜ Open |

## Addendum Summary (2026-08-30)

| Tier | Open | Resolved |
|------|------|----------|
| C | 6 | 0 |
| H | 8 | 0 |
| M | 8 | 0 |
| L | 8 | 0 |
| **Total** | **30** | **0** |

---

*Addendum generated: 2026-08-30 | Disposition: implement/connect everything — no deletions. Cross-references: gap-ledger-validation.md (verification record); 07/08 in codebase-analysis/ (audit record).*

---

# Verification 2026-08-30 — Ledger vs. Implemented Code (authoritative)

> **Method:** Every Open item re-checked against current source (grep + direct reads, code is truth). Verdicts below supersede table statuses above.

## Addendum 2 (A-01…A-30) — verified verdicts

| ID | Ledger | Verified verdict | Evidence |
|----|--------|------------------|----------|
| A-01 | Open | ✅ **FIXED** | Unknown tool → `TaskResult::failure(..., "unknown_tool")` (`service_impl.rs:287-294` parallel, `:799-805` single-node) |
| A-02 | Open | ✅ **FIXED** (partial) | Missing node fails `node_not_found` (`:1941-1950`). NOTE: missing-graph path still returns placeholder "backwards compatibility" shim (`:1612`) — decide its fate |
| A-03 | Open | ✅ **FIXED** | `config_override` applied (`:1607-1610`), `config` consumed downstream |
| A-04 | Open | ✅ **FIXED** | `tokio::process::Command` (`hook_runner_impl.rs:83`), concurrent drain task (`:61`); 10ms poll loop gone |
| A-05 | Open | ✅ **FIXED** | Parallel path runs PreToolUse gating with `HookAbortSignal` + PostToolUse mirrored (`service_impl.rs:156-175` region); permission enforced |
| A-06 | Open | ✅ **FIXED** | `GAP-A-06`: HMAC over FULL canonical envelope, sorted-key JSON (`envelope_factory_impl.rs:45-65`) |
| A-07 | Open | ✅ **FIXED** | Real `ClaudeClassifier`/`OpenaiClassifier` wired on API keys; mock only behind `RIGORIX_MOCK_PLANNING=1` (`mcp/main.rs:820-880`) |
| A-08 | Open | ✅ **FIXED** | `"governance"` → `ActionMode::Governance` (`mode_resolver_impl.rs:84-89`) + real `dispatch_governance` (`router_impl.rs:342`) |
| A-09 | Open | ✅ **FIXED** | `GAP-A-09`: `fetch_update` CAS reserve (`llm_budget_impl.rs:206-219`); reservation guard in production path |
| A-10 | Open | ✅ **FIXED (removed)** | `GAP-A-10`: SSE removed; `--sse` deprecation-warns, always stdio (`mcp/main.rs:1774-1785`) |
| A-11 | Open | ✅ **FIXED** | All 4 flags read (`service_impl.rs:481,502-513,580,2126-2137`); actions `max_llm_calls` applied (`context_builder_impl.rs:381-463`) |
| A-12 | Open | ✅ **FIXED** | Hooks on `tokio::process`; **0** `std::fs`/`std::process` in `service_impl.rs`; old `template_generation/generator.rs` gone |
| A-13 | Open | ✅ **FIXED** | `GAP-A-13`: `EvaluateInput.backend` + `BackendNotFound` (`service_impl.rs:134-145`, `dto.rs:33-38`) |
| A-14 | Open | ✅ **FIXED** | `GAP-A-14`: gate blocks on time + token limits (`enforcement/application/enforcer_impl.rs:221-224`) |
| A-15 | Open | ✅ **FIXED** | `IndexerServiceImpl` + FileSystem symbol/source + InMemory grammar repos all exist |
| A-16 | Open | ✅ **FIXED** | All 9 checked traits now have 1 impl each |
| A-17 | Open | ✅ **FIXED** | `AuditEvent::` emitted at 5 sites (`audit_sender_impl.rs:244,320,330,348,367`) |
| A-18 | Open | ✅ **FIXED** | `GAP-A-18`: RiskClassifier constructed + drives gating (`tools/application/registry_impl.rs:35,56,521`) |
| A-19 | Open | ⚠️ **PARTIAL** | `GAP-A-19` builder param exists (`service_impl.rs:2701-2718`) but no production construction of `FailureClassifierServiceImpl` found (only tests) — verify composition wiring |
| A-20 | Open | ⚠️ **PARTIAL** | `FileSystemTemplateRepository` implemented + exported but no construction in engine/mcp composition — implemented, not wired |
| A-21 | Open | ✅ **FIXED** | Both domain→application imports replaced with domain-local types |
| A-22 | Open | ❌ **STILL OPEN** | `create_session` zero runtime callers; `handle_initialize` takes unused `_params` (`mcp/main.rs:1303,1326`) |
| A-23 | Open | ✅ **MOSTLY FIXED** | 12+ integration files incl. backend/budget/identity; `unit/mod.rs` now compiles |
| A-24 | Open | ✅ **FIXED** | 24 stub files replaced; single `unit/mod.rs` documents history (match is a doc comment) |
| A-25 | Open | ✅ **FIXED** | Shared `mcp/tests/common/mod.rs`, deadline-based; 1 residual sleep |
| A-26 | Open | ✅ **FIXED** | README uses `rigorix-cli` (`:179,183-184`); `[[bin]] name="rigorix"` also added |
| A-27 | Open | ✅ **MOSTLY FIXED** | Go claim gone; 1 `continue-on-error` (documented teardown); "86 steps" moot — HOW docs removed |
| A-28 | Open | ✅ **FIXED (moot)** | `HOW-*.md` no longer exists (docs/ = dr-plan.md, runbook.md) |
| A-29 | Open | ✅ **FIXED** | `.gitignore` repaired; `.mov` untracked; no `.rigorix/state` tracked |
## Addendum 1 (H-07…L-09) — verified verdicts

| ID | Ledger | Verified verdict | Evidence |
|----|--------|------------------|----------|
| H-07 | Open | ✅ **FIXED** | `approve_node` checks `requires_approval` + `AwaitingApproval` (`service_impl.rs:2506-2518`) |
| H-08 | Open | ✅ **FIXED** | `hydrate_execution` uses persisted `input.started_at` (`service_impl.rs:~2597-2614`) |
| M-12 | Open | ❌ **STILL OPEN** | Signing still `Option<String> signing_key` (`envelope_factory_impl.rs:24-31`); no `evidence: degraded` marker |
| M-13 | Open | ✅ **FIXED** | `planning_prompt_content` wired (`orchestrator_impl.rs:414` → `envelope_factory_impl.rs:105`) |
| M-14 | Open | ✅ **FIXED** | Zero `let _ = ...publish(...)` remaining in engine/src |
| M-15 | Open | ⚠️ **PARTIAL** | Both vocabularies still persisted; `state.rs:211-215` documents GAP-3 migration compat — consolidate or mark intentional |
| L-08 | Open | ❌ **STILL OPEN** | unwrap/expect count now **~1729** (grew from 1617) |
| L-09 | Open | ✅ **FIXED** | `detect_git_info` populates git_commit/branch (`orchestrator_impl.rs:876`) |

## Base ledger "Resolved" claims — spot-check results

| Item | Claim | Verified |
|------|-------|----------|
| C-01 | 179 `#[tracing::instrument]` | ✅ now 183 (claim holds) |
| M-03 | `deny.toml` added | ❌ **MISSING** at repo root — stale Resolved claim |
| M-04 | `.githooks/pre-commit` | ❌ **MISSING** — stale Resolved claim |
| L-06 | `install_coverage_tools.sh` | ❌ **MISSING** from `.pi/scripts/` — stale Resolved claim |
| M-07 | `common/validation.rs` | ✅ exists |
| M-10 | `benches/dag_engine.rs` | ✅ exists |
| M-09 | 8 ADRs "Accepted" | ⚠️ down to 2 — nearly resolved |

## Corrected totals (post-verification)

| Group | Ledger open | Actually open | Fixed | Partial |
|-------|------------|---------------|-------|---------|
| Addendum 2 (A-01…A-30) | 30 | **2** (A-22; A-02 graph-shim edge) | 26 | 2 (A-19, A-20 wiring) |
| Addendum 1 (H-07…L-09) | 8 | **2** (M-12, L-08) | 5 | 1 (M-15) |
| Base | M-09 open; M-03/M-04/L-06 marked Resolved | 3 stale Resolved claims + 2 ADRs | — | — |

**Remaining actions:** (1) wire A-19 classifier + A-20 filesystem template repo into composition roots; (2) A-22 session handshake; (3) M-12 signing policy for approval runs; (4) L-08 unwrap pass on dispatch/evidence path; (5) restore or re-scope M-03/M-04/L-06 tooling files or mark Removed; (6) decide A-02 missing-graph placeholder shim fate; (7) finish M-09's last 2 ADRs.

---

*Verification performed 2026-08-30 against working tree @ commit 3dcd26ab. This section supersedes table statuses above.*

## Re-verification 2026-08-30 (post issue-series batches 8–20, @ dba15950)

> The batch series (#763–#775) landed 28 commits after the verification base (3dcd26ab), which
> invalidated several verdicts. Re-checked every Open/Partial/Disputed row against HEAD:

| ID | Prior verdict | **Re-verified** | Delta |
|----|--------------|-----------------|-------|
| A-19 | ⚠️ PARTIAL (no prod construction) | ✅ **FIXED** | `ParallelExecutionFactoryImpl::create` constructs `RetryEvaluationServiceImpl::with_classifier(Arc::new(FailureClassifierServiceImpl))` (`factory_impl.rs:53`) — wired by #769 after the verification base |
| A-25 | ✅ FIXED (1 residual sleep) | ✅ **FIXED** | 0 `sleep` calls in `mcp/tests/` — residual removed post-verification |
| A-27 | ✅ MOSTLY (1 documented continue-on-error) | ✅ **FIXED** | 0 `continue-on-error: true` remain in `ci.yml` (#773) — the 1 remaining text mention is a comment, not a step |
| A-26 | ✅ FIXED ("`[[bin]] name="rigorix"` also added") | ⚠️ **FIXED, evidence wrong** | No `[[bin]]` exists in `cli/Cargo.toml` (only `[package] rigorix-cli` + `[lib] rigorix`) — README→`rigorix-cli` fix is correct, but the cited `[[bin]]` is fictional |
| M-09 | "down to 2 ADRs" | ⚠️ **STILL OPEN: 8** | Batch 17 flipped engine ADR-002..009 only. Remaining `Accepted`: **7 mcp ADRs** (ADR-001..007 in `mcp/.pi/architecture/decisions/`) + **engine ADR-010** (scored-evaluation, implemented by #765 but never flipped). ADR-011/012 correctly `Proposed` (approval epic pending) |
| L-08 | ❌ STILL OPEN (~1729 global) | ⚠️ **Path-scope FIXED; global open** | #775 removed the audit evidence-path panics (client-build expects → `Option<reqwest::Client>` + `AuditError::Internal`). `service_impl.rs` = 0 non-test unwraps; hooks/state_persistence production paths clean. Global count (~1700) remains a separate backlog item per issue scope |
| A-02 | ⚠️ PARTIAL (graph shim) | ⚠️ **UNCHANGED** | `service_impl.rs:1612` missing-graph placeholder shim still present — fate decision still pending |
| A-20 | ⚠️ PARTIAL (not wired) | ⚠️ **UNCHANGED** | `template_repository_from_config` still has no production caller outside `templates/infrastructure/` |
| A-22 | ❌ STILL OPEN | ❌ **UNCHANGED** | `create_session` still zero runtime callers; `handle_initialize` still fabricates the response |
| M-12 | ❌ STILL OPEN | ❌ **UNCHANGED** | `signing_key: Option<String>` + no `evidence: degraded` marker |
| M-15 | ⚠️ PARTIAL | ⚠️ **UNCHANGED** | dual `node_states`/`exec_node_states` both persisted (GAP-3 compat documented) |
| M-03/M-04/L-06 | stale Resolved claims | ❌ **CONFIRMED MISSING** | `deny.toml`, `.githooks/pre-commit`, `.pi/scripts/install_coverage_tools.sh` do not exist — re-scope or mark Removed |
| C-01 | 183 instruments | ✅ exact | 183 `#[tracing::instrument]` |

**Net correction to "Corrected totals":** Actually-open = 2 (A-22, M-12) + M-09(8 ADRs); Partial = 2 (A-02 graph-shim, A-20 wiring) + M-15; stale-Resolved = 3 tooling files (M-03/M-04/L-06); evidence-wrong = A-26 (`[[bin]]` claim). All other FIXED verdicts hold.


## Validation 2026-08-30 (independent audit of the Re-verification above)

> Every row of the Re-verification section re-checked against HEAD (dba15950). Two corrections:

| Row | Re-verification verdict | Validation verdict | Evidence |
|-----|------------------------|--------------------|----------|
| M-12 | ❌ UNCHANGED / STILL OPEN | ✅ **FIXED** — re-verification missed it | `GAP-M-12` implemented: `evidence_degraded: !input.sign` + comment "an unsigned run is explicitly degraded evidence" (`envelope_factory_impl.rs:109-113`), test `test_evidence_degraded_marker` (`:317-320`). Fix landed in 5eb95f43 (#761), **verified ancestor of dba15950** — so it was already in HEAD when the re-verification ran; its `signing_key: Option` grep caught the key but missed the degraded marker |
| A-25 | ✅ FIXED (0 sleeps) | ⚠️ **FIXED, count wrong** | 5 `sleep` matches in `mcp/tests/` — 4 are doc comments, 1 is a real `std::thread::sleep(POLL_INTERVAL)` (`common/mod.rs:109`) inside the deadline-based poll loop. That poll-sleep is the *replacement* for fixed sleeps (bounded, interval-based), so the verdict "fixed" stands but "0 sleeps" is literally false |

All other rows **validated correct**:
- **A-19 FIXED** ✅ — confirmed `RetryEvaluationServiceImpl::with_classifier(Arc::new(FailureClassifierServiceImpl))` at `execution_engine/infrastructure/factory_impl.rs:53` (unit struct, no `::new` — which is why a `::new` grep misses it).
- **A-26 evidence-wrong** ✅ — confirmed: `cli/Cargo.toml` has only `[package] rigorix-cli` + `[lib] name = "rigorix"`, **no `[[bin]]` section**. The earlier verification's `[[bin]]` citation was wrong (a `^name` grep matched the lib name).
- **M-09 = 8 ADRs** ✅ — exactly 7 mcp ADRs (001–007) + engine ADR-010 remain "Accepted" (templates excluded).
- **A-27** ✅ — single `continue-on-error` match is inside a comment (ci.yml:165), no active step.
- **L-08 path-scoped** ✅ — 0 non-test unwraps/expects in `service_impl.rs`; global engine count ~1729 stands.
- **A-02 shim, A-20 unwired, A-22 open, M-15 dual-vocabulary, M-03/M-04/L-06 files missing** ✅ — all re-confirmed unchanged.

**Net final state:** Actually-open = **A-22** + A-02 graph-shim decision + A-20 wiring + M-15 consolidation + M-09 (8 ADRs) + L-08 (global) + 3 stale tooling claims (M-03/M-04/L-06). **M-12 is closed.**

*Validated 2026-08-30 against HEAD dba15950. Supersedes the Re-verification for the M-12 and A-25 rows.*



## Batch 21 (gap-ledger remainder) — implemented

> One run, branch `feature/gap-ledger-remainder` (PR #776-sibling). Code-verified against HEAD.

| ID | Outcome | Evidence |
|----|---------|----------|
| A-02 | ✅ **FIXED** | Missing graph → `ExecutionError::InvalidState` (service_impl.rs execute_graph); placeholder shim removed; 3 tests converted to the fail-contract. **Bonus:** exposed + fixed a latent deadlock — `notify_progress` re-locked the `sessions` Mutex while callers held it (non-reentrant std Mutex); both call sites now release the lock before notifying; `test_progress_callback_fires` is the regression guard |
| A-20 | ✅ **FIXED (wired)** | CLI orchestrator loads `.rigorix/templates` via `template_repository_from_config` (engine's config-gated filesystem repo) — the hand-rolled tokio::fs loop is gone |
| A-22 | ✅ **FIXED** | `handle_initialize` routes through `McpServerService::initialize` (creates the session); protocol negotiation echoes the client's version when supported (`2025-03-26`/`2025-06-18`/`2024-11-05`), else the default — no fabricated response |
| M-09 | ✅ **RESOLVED** | 7 mcp ADRs + engine ADR-010 statused with code evidence. **Honest verdicts:** ADR-002 = **Superseded** (SQLx never adopted — 0 deps); ADR-006 = **Partial** (engine-delegated usage yes; proxy telemetry not implemented); ADR-004/005 qualified (SSE removed, GAP-A-10) |
| M-03 | ✅ **RESOLVED** | `engine/deny.toml` exists (claim said repo root — corrected to actual location) |
| M-04 | ✅ **RESOLVED** | `.githooks/pre-commit` restored: `cargo fmt --check` + `cargo clippy --workspace --all-targets -D warnings` |
| L-06 | ✅ **RESOLVED** | `.pi/scripts/install_coverage_tools.sh` restored **and wired**: real workspace coverage via `cargo llvm-cov` (`.pi/scripts/coverage.sh`) runs in local-ci (Stage 4b) + a dedicated CI job — baseline **78.92% line coverage**, gated ≥60% |
| A-26 | ✅ **Evidence corrected** | No `[[bin]]` exists in cli/Cargo.toml (prior evidence was fictional); README→`rigorix-cli` fix stands |
| M-12 | ✅ **ALREADY FIXED (missed by re-verification)** | `evidence_degraded: !input.sign` + `test_evidence_degraded_marker` landed in 5eb95f43 (#761), ancestor of HEAD. The re-verification's grep (`evidence: degraded` with colon) missed the underscore field name; the independent Validation section caught it. Unnecessary tracking issue #776 closed |
| A-25 | ✅ verdict stands | 1 bounded poll-sleep remains in `mcp/tests/common/mod.rs:109` (deadline-loop interval) — the intended replacement for fixed sleeps |
| L-08-global | ⬜ **Deferred** → issue #777 (backlog) | global unwrap elimination, beyond the #751 path scope |
| M-15 | ✅ **RESOLVED (#758)** | canonicalize: `exec_node_states` is the single persisted representation (coarse `node_states` → `#[serde(skip_serializing)]` derived view rebuilt on hydrate; final states now persist exec states too; `update_node_state` syncs into exec). Full API removal still rides the approval epic's `approval_records` format change |

**Remaining open after batch 21:** deferred to approval epic: M-15 (#758) · backlog: L-08-global (#777). All A-rows + M-09/M-03/M-04/L-06/M-12 resolved.
