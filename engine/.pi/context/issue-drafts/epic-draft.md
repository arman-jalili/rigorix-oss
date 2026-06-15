---
guardian_epic:
  id: EPIC-001
  title: "Observability Foundation"
  status: planned
  priority: critical
  created_at: "2026-06-15"

  description: |
    Add structured tracing to every service method, a centralized health aggregator,
    and Prometheus metrics endpoints so the system is observable in production.

  goals:
    - "Every public service method has #[tracing::instrument] with appropriate spans"
    - "Centralized HealthService aggregates status from all 16 HTTP modules"
    - "Prometheus /metrics endpoint exposes budget consumption, retry rates, execution latency"
    - "All 13 modules without health endpoints gain them"

  gaps_addressed:
    - "C-01: No logging/tracing infrastructure"
    - "C-02: No centralized health service and no /metrics endpoints"

  issue_ids:
    - "EPIC-001-ISSUE-001: Add tracing crate + instrument all services"
    - "EPIC-001-ISSUE-002: Centralized HealthService"
    - "EPIC-001-ISSUE-003: Prometheus /metrics endpoints"
    - "EPIC-001-ISSUE-004: Health endpoints for remaining 13 modules"

  validator_conditions:
    - "Implement SpanPrivacy filter (no secrets/PII in tracing spans) [Security]"
    - "/metrics endpoint must have access control (internal network or auth header) [Security]"
    - "100% #[instrument] coverage on all public service methods [Operations]"
    - "/metrics with budget consumption rate, retry frequency, latency histogram [Operations]"

  dependencies:
    - name: none
      type: internal
      note: "Root epic — no internal dependencies"

  timeline:
    start: "2026-06-17"
    target: "2026-06-28"
    effort: "1-2 weeks"

---

# EPIC-001: Observability Foundation

## Description

Add structured tracing, centralized health aggregation, and Prometheus metrics to the entire Rigorix system. This epic addresses the two Critical gaps (C-01, C-02) from the Gap Ledger.

## Goals

1. Every public service method has `#[tracing::instrument]` with appropriate spans
2. Centralized `HealthService` aggregates status from all 16 HTTP modules
3. Prometheus `/metrics` endpoint exposes budget consumption, retry rates, execution latency
4. All 13 modules without health endpoints gain them

## Issues (in dependency order)

1. **[EPIC-001-ISSUE-001]** Add tracing crate + instrumentation — Contract/Infrastructure
2. **[EPIC-001-ISSUE-002]** Centralized HealthService — Service
3. **[EPIC-001-ISSUE-003]** Prometheus /metrics endpoints — Service
4. **[EPIC-001-ISSUE-004]** Health endpoints for remaining 13 modules — Handler

## Labels

- `risk::critical`
- `type::feature`
- `layer::cross-cutting`

## Validator Conditions (pre-embedded)

- SpanPrivacy filter — no secrets/PII in tracing spans
- `/metrics` endpoint access control
- 100% `#[instrument]` coverage on all public service methods
- `/metrics` with minimum 3 metric types

---

---
guardian_epic:
  id: EPIC-002
  title: "Architecture & Code Quality"
  status: planned
  priority: high
  created_at: "2026-06-15"

  description: |
    Fix module boundary violations (classifiers in domain layer), eliminate the
    stale execution stub, clean up 24 compiler warnings, and improve error
    retriability logic for production reliability.

  goals:
    - "All LLM implementations (ClaudeClassifier, OpenAIClassifier) in infrastructure layer"
    - "execution module merged into execution_engine — no duplicate module"
    - "Zero compiler warnings — cargo build with -D warnings passes"
    - "Each domain error self-declares retriable variants"

  gaps_addressed:
    - "H-01: 24 compiler warnings"
    - "H-04: Classifiers in domain layer"
    - "H-05: execution module is a stub"
    - "H-06: is_retriable() too conservative"

  issue_ids:
    - "EPIC-002-ISSUE-001: Move classifiers out of domain layer (H-04)"
    - "EPIC-002-ISSUE-002: Merge execution stub into execution_engine (H-05)"
    - "EPIC-002-ISSUE-003: Fix 24 compiler warnings (H-01)"
    - "EPIC-002-ISSUE-004: Per-module is_retriable() delegation (H-06)"

  dependencies:
    - name: none
      type: internal
      note: "Root epic — no internal dependencies. Unblocks EPIC-003."

  timeline:
    start: "2026-06-17"
    target: "2026-06-25"
    effort: "5-7 days"

---

# EPIC-002: Architecture & Code Quality

## Description

Clean up architectural violations and code quality issues. Fix module boundary violations (H-04), eliminate the stale `execution` module (H-05), fix 24 compiler warnings (H-01), and improve error retriability (H-06). This epic is a prerequisite for EPIC-003 (Testing Hardening).

## Goals

1. All LLM implementations in infrastructure layer — domain contains only interfaces
2. `execution` module merged into `execution_engine` — single authoritative module
3. Zero compiler warnings — `#![deny(warnings)]` can be enabled in CI
4. Each domain error self-declares which variants are retriable

## Issues (in dependency order)

1. **[EPIC-002-ISSUE-001]** Move classifiers out of domain layer — Architecture
2. **[EPIC-002-ISSUE-002]** Merge execution stub into execution_engine — Architecture
3. **[EPIC-002-ISSUE-003]** Fix 24 compiler warnings — Code Quality
4. **[EPIC-002-ISSUE-004]** Per-module is_retriable() delegation — Code Quality

## Labels

- `risk::high`
- `type::refactor`
- `layer::cross-cutting`

## Architecture Note

After EPIC-002, update ADR-003 to reflect actual file layout.

---

---
guardian_epic:
  id: EPIC-003
  title: "Testing Hardening"
  status: planned
  priority: high
  created_at: "2026-06-15"

  description: |
    Add concurrent-safety tests, cross-module integration tests, and live LLM
    API integration tests so the system's correctness is verified under realistic
    conditions. Depends on EPIC-002 for clean architecture.

  goals:
    - "Concurrent-safety tests for budget_tracking, dag_engine, execution_engine, state_persistence, event_system"
    - "engine/tests/ directory with plan→execute→audit integration test"
    - "Live LLM API integration tests behind #[cfg(feature = "live-tests")]"

  gaps_addressed:
    - "C-03: No concurrent-safety tests"
    - "H-02: No cross-module integration test suite"
    - "H-03: No live LLM API integration tests"

  issue_ids:
    - "EPIC-003-ISSUE-001: Concurrent-safety tests (C-03)"
    - "EPIC-003-ISSUE-002: Cross-module integration test suite (H-02)"
    - "EPIC-003-ISSUE-003: Live LLM API integration tests (H-03)"

  dependencies:
    - name: EPIC-002
      type: internal
      note: "Architecture cleanup first — imports will change in EPIC-002"

  timeline:
    start: "2026-06-25"
    target: "2026-07-04"
    effort: "1-2 weeks"

---

# EPIC-003: Testing Hardening

## Description

Add concurrent-safety tests (C-03), cross-module integration tests (H-02), and live LLM API integration tests (H-03). Depends on EPIC-002 for clean architecture.

## Goals

1. Five modules have dedicated concurrent-safety test suites
2. `engine/tests/` directory with 3 integration test files
3. Live LLM API tests behind feature flag for optional CI execution

## Issues (in dependency order)

1. **[EPIC-003-ISSUE-001]** Concurrent-safety tests — Testing
2. **[EPIC-003-ISSUE-002]** Cross-module integration test suite — Testing
3. **[EPIC-003-ISSUE-003]** Live LLM API integration tests — Testing

## Labels

- `risk::high`
- `type::test`
- `layer::testing`

## Validator Conditions (pre-embedded)

- Use environment variables only for live API keys, never hardcoded [Security]
- Must create `engine/tests/` with 3 integration tests [Operations]
- Must include concurrency tests for all 5 specified modules [Operations]
