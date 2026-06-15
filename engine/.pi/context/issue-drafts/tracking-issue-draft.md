---
guardian_issue:
  id: TRACKING-001
  title: "Tracking: Production Readiness"
  type: tracking
  status: planned
  priority: critical
  created_at: "2026-06-15"

  purpose: |
    This tracking issue monitors the overall progress of the Production Readiness
    initiative — 3 epics addressing 9 gaps (3 Critical, 6 High) from the Gap Ledger.

  related_epics:
    - "EPIC-001: Observability Foundation (C-01, C-02)"
    - "EPIC-002: Architecture & Code Quality (H-01, H-04, H-05, H-06)"
    - "EPIC-003: Testing Hardening (C-03, H-02, H-03)"
---

# Tracking: Production Readiness

## Purpose

Monitors progress of the Production Readiness initiative.

## Epic Sequence

```
EPIC-002 (Architecture & Code Quality) ──────► EPIC-001 (Observability Foundation)
     │
     └─────────────────────────────────────────► EPIC-003 (Testing Hardening)
```

## Issues Checklist

### EPIC-002: Architecture & Code Quality
- [ ] EPIC-002-ISSUE-001: Move classifiers out of domain layer — Status: planned
- [ ] EPIC-002-ISSUE-002: Merge execution stub into execution_engine — Status: planned
- [ ] EPIC-002-ISSUE-003: Fix 24 compiler warnings — Status: planned
- [ ] EPIC-002-ISSUE-004: Per-module is_retriable() delegation — Status: planned

### EPIC-001: Observability Foundation
- [ ] EPIC-001-ISSUE-001: Add tracing crate + instrument all services — Status: planned
- [ ] EPIC-001-ISSUE-002: Centralized HealthService — Status: planned
- [ ] EPIC-001-ISSUE-003: Prometheus /metrics endpoints — Status: planned
- [ ] EPIC-001-ISSUE-004: Health endpoints for remaining 13 modules — Status: planned

### EPIC-003: Testing Hardening
- [ ] EPIC-003-ISSUE-001: Concurrent-safety tests — Status: planned
- [ ] EPIC-003-ISSUE-002: Cross-module integration test suite — Status: planned
- [ ] EPIC-003-ISSUE-003: Live LLM API integration tests — Status: planned

## Progress

- Total Issues: 11
- Completed: 0/11 (0%)
- In Progress: 0/11 (0%)

## Validator Conditions to Track

| # | Condition | Applies To | Met? |
|---|-----------|-----------|------|
| 1 | SpanPrivacy filter (no secrets/PII in traces) | EPIC-001-ISSUE-001 | ❌ |
| 2 | /metrics access control | EPIC-001-ISSUE-003 | ❌ |
| 3 | 100% #[instrument] coverage on public services | EPIC-001-ISSUE-001 | ❌ |
| 4 | /metrics with 3 metric types | EPIC-001-ISSUE-003 | ❌ |
| 5 | Env-only API keys for live tests | EPIC-003-ISSUE-003 | ❌ |
| 6 | engine/tests/ with 3 integration tests | EPIC-003-ISSUE-002 | ❌ |
| 7 | Concurrency tests for 5 modules | EPIC-003-ISSUE-001 | ❌ |
| 8 | Create ADR-009 for observability | Pre EPIC-001 | ❌ |

## Timeline

- Start: 2026-06-17
- EPIC-002 Target: 2026-06-25 (5-7 days)
- EPIC-001 Target: 2026-06-28 (1-2 weeks)
- EPIC-003 Target: 2026-07-04 (1-2 weeks)
- Target Completion: 2026-07-04
