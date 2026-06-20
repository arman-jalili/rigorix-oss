# Disaster Recovery Plan: plan-validation Module

<!--
Canonical Reference: .pi/architecture/modules/plan-validation.md
Last Updated: 2026-06-19
-->

## Overview

This document covers disaster recovery procedures for the plan-validation module,
which orchestrates the self-correcting template validation loop. The module is
stateless per session — all state is transient (in-memory `ValidationState`) and
persisted only through optional `ValidationReportRepository` and
`ValidatedTemplateRepository` implementations.

## Failure Scenarios

### Scenario 1: Validation Loop Hangs or Times Out

**Symptoms:**
- Validation takes longer than expected (no budget progress)
- Iterations stall on LLM call or execution engine
- Orchestrator timeout is hit

**Impact:** Current validation session fails; downstream template execution blocked.

**Recovery:**
1. Cancel validation via cancellation module (`ValidationLoopError::Cancelled`)
2. Verify LLM API key and endpoint in planning pipeline configuration
3. Check execution engine connectivity
4. Reduce `max_iterations` to 2 and `max_cumulative_tokens` to 25,000 for faster failure
5. Retry with reduced config; if still failing, escalate to human review

### Scenario 2: Budget Exhaustion on Production Template

**Symptoms:**
- `ValidationOutcome::BudgetExhausted` returned
- Failed to validate a template that previously passed
- Token usage spiked on retries

**Impact:** Production template cannot be validated — block deployments.

**Recovery:**
1. Check `max_cumulative_tokens` (default: 50,000) — increase to 100,000 for complex templates
2. Verify LLM model hasn't changed to a more expensive variant
3. Check failure history — repeated failures inflate token usage
4. If template was previously validated, use cached version from `ValidatedTemplateRepository`
5. Escalate: reduce LLM model complexity (e.g., switch from claude-sonnet to claude-haiku)

### Scenario 3: Repeated Identical Failures (LLM Not Learning)

**Symptoms:**
- `ContextAugmenter::check_repeated_failures()` returns `is_repeated: true`
- Same error pattern across multiple retry iterations
- LLM output shows no improvement despite augmented context

**Impact:** All retries exhausted; template marked as `Failed`.

**Recovery:**
1. Inspect `ValidationReport.failure_history` for the repeated failure pattern
2. Check if the failure is deterministic (cannot be fixed by LLM alone)
3. Apply manual fix to template and re-validate
4. If the template requires external context not available to the LLM, update prompt
5. Consider escalating to human template author for structural template redesign

### Scenario 4: Repository Corruption or Data Loss

**Symptoms:**
- `ValidationReportRepository` returns errors
- `ValidatedTemplateRepository` cache entries missing
- Validation reports cannot be saved or loaded

**Impact:** Loss of validation history; inability to cache validated templates.

**Recovery:**
1. Ran against local filesystem; no external database dependency
2. If `ValidatedTemplateRepository` cache is lost, re-run validation (template will pass again)
3. If `ValidationReportRepository` data is lost, audit trail for that session is unavailable
4. Enable persistent storage backend (e.g., SQLite, S3) in repository implementation
5. Set up periodic backups of report storage

### Scenario 5: ContextAugmenter Produces Malformed Augmented Context

**Symptoms:**
- Augmented intent contains garbled or incomplete failure text
- LLM produces nonsensical output on retry
- `format_failure()` produces unparseable format

**Impact:** Retry iteration fails with new errors; validation loop degrades.

**Recovery:**
1. Check `TemplateFailure` variant — unknown variants fall back to `Debug` format
2. Verify `ContextAugmenter::format_failure()` handles all 6 failure variants:
   - MissingSymbol, WrongArgCount, TypeMismatch, CompileError, AssertionFailure, TestFailure
3. Update `format_failure()` match arms if new failure variants are added
4. Add input validation for failure content before formatting

## Backup Strategy

| Data | Backup Method | Frequency | Retention |
|------|--------------|-----------|-----------|
| Validation reports | Via `ValidationReportRepository` | Per-session | 30 days (configurable) |
| Validated templates | Via `ValidatedTemplateRepository` | On successful validation | Indefinite (until cache invalidation) |
| `ValidationLoopConfig` | Configuration source (config files, env) | Per-deployment | Version-controlled |

## RTO/RPO Targets

| Metric | Target | Notes |
|--------|--------|-------|
| RTO (Recovery Time Objective) | < 5 minutes | Module is stateless; restart is instant |
| RPO (Recovery Point Objective) | N/A | No mutable state to lose per session |
| MTPD (Maximum Tolerable Period of Disruption) | < 30 minutes | Templates must be validated within CI pipeline window |

Since the module is entirely stateless (state is per-session and in-memory), RTO is
bounded only by the time to restart the service and re-establish LLM connections.
The `ValidationLoopConfig` is loaded fresh from configuration on each start.

## Failover Plan

1. **Single instance failure** — The orchestrator detects the failure and retries with
   a fresh `ValidationLoopImpl` instance
2. **LLM API failure** — The planning pipeline handles retries internally;
   validation loop treats as temporary error
3. **Execution engine failure** — Validation loop returns `ValidationLoopError::ExecutionError`;
   orchestrator should retry after verifying execution engine health

## Testing the DR Plan

1. **Unit tests** — All validation loop components have unit tests covering success,
   failure, budget exhaustion, and cancellation paths
2. **Simulated failures** — Use `MockQualityGate` to test budget exhaustion and retry paths
3. **Integration tests** — Full validation loop with mock planning and execution services
4. **E2E tests** — Full TypeScript demo: intent → validate → corrected template

## See Also
- [Runbook](./runbook-plan-validation.md)
- [Architecture Module](../.pi/architecture/modules/plan-validation.md)
- [Template System DR Plan](./dr-plan-template-system.md)
