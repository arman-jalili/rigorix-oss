# Validator Reports — Production Readiness Epic Plan

**Task:** Epic Plan Overview — Production Readiness (3 epics covering C-01, C-02, C-03, H-01, H-02, H-03, H-04, H-05, H-06)
**Scope:** Moderate
**Files Reviewed:** Gap Ledger, all 17 module architecture docs, src/error.rs, src/planning/domain/, src/execution/

---

## Architecture Validator Report

**Results:**

| Check | Status | Notes |
|-------|--------|-------|
| ADR compliance — EPIC-001 (tracing) | ✅ | No existing ADR on observability; new ADR recommended |
| ADR compliance — EPIC-001 (health/metrics) | ✅ | No existing ADR on health endpoints; follows existing pattern from 3 modules |
| ADR compliance — EPIC-002 (H-04 move classifiers) | ✅ | ADR-003 (LLM Provider Traits) allows trait-based abstraction; moving impls to infrastructure aligns with Clean Architecture intent |
| ADR compliance — EPIC-002 (H-05 merge execution) | ✅ | No ADR conflicts; resolves module ambiguity |
| ADR compliance — EPIC-002 (H-06 retriable) | ✅ | ADR-007 (Risk Gating) and ADR-001 (Error Handling) don't prescribe retriable logic; safe to extend |
| Module boundaries respected | ✅ | All changes contained within module boundaries |
| Dependency direction correct | ✅ | No inward violations |
| Error handling pattern followed | ✅ | Uses existing `#[from]` delegation pattern |
| Naming conventions consistent | ✅ | Follows existing codebase conventions |

**Relevant ADRs Reviewed:**
- **ADR-001** (Architecture Pattern) — ✅ Compliant. DDD modular monolith maintained.
- **ADR-003** (LLM Provider Traits) — ✅ Compliant. H-04 _improves_ compliance by moving impls out of domain.
- **ADR-005** (Event Bus) — ✅ Compliant. EPIC-001 tracing will additionally emit to EventBus.
- **ADR-007** (Risk Gating) — ✅ Compliant. EPIC-002 H-06 extends, doesn't conflict.
- **ADR-008** (RAII Budget) — ✅ Compliant. Not affected.

**Issues:**

| Severity | Description | Fix |
|----------|-------------|-----|
| Low | EPIC-001 introduces `tracing` — warrants ADR or ADR update documenting observability decisions | Create ADR-009: Observability Strategy (tracing + metrics) |

**Verdict:**
- [x] APPROVED
- [ ] APPROVED WITH CONDITIONS
- [ ] REQUIRES CHANGES
- [ ] REJECTED

**Recommendations:**
1. Create ADR-009 for observability strategy before EPIC-001 implementation begins
2. After EPIC-002, update ADR-003 to reflect actual file layout (ClaudeClassifier → infrastructure/)

---

## Security Validator Report

**Results:**

| Check | Status | Notes |
|-------|--------|-------|
| Injection vulnerabilities | ✅ | No user input in shell commands, no SQL, no template injection in scope |
| Authentication / Authorization | ✅ | No auth changes in scope |
| Secret leakage | ⚠️ | EPIC-003 (H-03) live LLM tests need API keys — risk of key exposure in test code |
| Unsafe operations | ✅ | No unsafe deserialization, weak crypto, or insecure randomness in scope |
| Data handling | ✅ | Tracing (EPIC-001) must not log PII or API keys |
| Hardcoded secrets | ✅ | No new hardcoded secrets proposed |
| Path traversal | ✅ | Not affected by any proposed change |

**Issues:**

| Severity | File/Component | Description | Fix |
|----------|---------------|-------------|-----|
| Medium | EPIC-003 — Live API tests | API keys for Claude/OpenAI integration tests could leak if stored in test files | Use environment variables only, add to `.gitignore` for any test config files, use `#[cfg(feature = "live-tests")]` + CI secrets only |
| Medium | EPIC-001 — Tracing | Tracing instrumentation could inadvertently log API keys, tokens, or user intent text | Implement `SpanPrivacy` filter — redact fields matching patterns (`api_key`, `token`, `secret`, `password`). Add to Security section of tracing ADR. |
| Low | EPIC-001 — Metrics | `/metrics` endpoint exposes operational data that could aid attackers | Restrict `/metrics` to internal network or require `Metrics-Auth` header |

**Verdict:**
- [x] APPROVED WITH CONDITIONS
- [ ] APPROVED
- [ ] REQUIRES CHANGES
- [ ] REJECTED

**Conditions:**
1. EPIC-001 must implement span privacy filtering (no secrets/PII in tracing spans)
2. EPIC-003 live tests must use environment variables only, never hardcoded keys
3. EPIC-001 `/metrics` endpoint must have access control (internal network or auth header)

---

## Operations Validator Report

**Results:**

| Check | Status | Notes |
|-------|--------|-------|
| Performance concerns | ✅ | No O(N²) or pathological patterns in scope |
| Observability — tracing | ❌ | **0 instances** of `#[instrument]` found across all 17 modules (EPIC-001 required) |
| Observability — health | ❌ | Only 3/16 HTTP modules have health endpoints (EPIC-001 required) |
| Observability — metrics | ❌ | **0 instances** of Prometheus or any metrics framework (EPIC-001 required) |
| Cancellation handling | ✅ | 7 files use `cancel_token`; cancellation integration is mature |
| Atomic writes | ✅ | 7 files use `fs::rename` atomic pattern (state_persistence, tools, audit) |
| Resource management | ✅ | Bounded channels, Drop implementations, RAII patterns present |
| Benchmarks | ❌ | **No `benches/` directory** exists — performance risks documented but unmeasured |
| Integration tests | ❌ | **No `tests/` directory** — critical paths untested end-to-end (EPIC-003 required) |
| Concurency safety tests | ❌ | **0 tests** use `tokio::spawn` or `try_join!` in test code (EPIC-003 required) |

**Automated Checks Summary:**

| Check | Result |
|-------|--------|
| `#[instrument]` occurrences | 0 across 17 modules |
| `cancel_token` occurrences | 7 files |
| `fs::rename` atomic writes | 7 files (state_persistence, tools, audit) |
| `benches/` directory | Does not exist |
| `tests/` directory | Does not exist |
| `tokio::spawn` in tests | 0 |

**Issues:**

| Severity | Component | Description | Fix |
|----------|-----------|-------------|-----|
| Critical | All 17 modules | Zero tracing instrumentation — no diagnostic output for production debugging | EPIC-001, Issue 1 |
| Critical | 13/16 HTTP modules | Missing health endpoints — no way to probe liveness/readiness | EPIC-001, Issue 4 |
| Critical | All modules | Zero metrics — no visibility into budget consumption, retry rates, latency | EPIC-001, Issue 3 |
| High | Project-wide | No integration tests — critical flows (plan→execute→audit) never tested end-to-end | EPIC-003, Issue 2 |
| High | 5 modules | No concurrency safety tests — RwLock, Mutex, atomic operations untested under contention | EPIC-003, Issue 1 |
| Medium | Project-wide | No benchmarks — DAG execution performance risk documented but unmeasured | Future epic (M-10) |

**Verdict:**
- [x] APPROVED WITH CONDITIONS
- [ ] APPROVED
- [ ] REQUIRES CHANGES
- [ ] REJECTED

**Conditions:**
1. EPIC-001 must achieve 100% `#[instrument]` coverage on all public service methods
2. EPIC-001 must add `/metrics` with at minimum: budget consumption rate, retry frequency, execution latency histogram
3. EPIC-003 must create `engine/tests/` with plan→execute→audit integration test
4. EPIC-003 must include concurrent-safety tests for budget_tracking, dag_engine, execution_engine, state_persistence, and event_system

---

## Summary

| Validator | Status | Conditions |
|-----------|--------|-----------|
| **Architecture** | ✅ APPROVED | 1 recommendation (create ADR-009 for observability) |
| **Security** | ✅ APPROVED WITH CONDITIONS | 3 conditions (span privacy, env-only API keys, metrics access control) |
| **Operations** | ✅ APPROVED WITH CONDITIONS | 4 conditions (100% instrument coverage, /metrics, integration tests, concurrency tests) |

**Decision: CONDITIONAL APPROVED** — Address conditions before implementation, proceed to `/issue-draft`.
