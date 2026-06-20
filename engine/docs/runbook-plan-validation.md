# Runbook: plan-validation Module

<!--
Canonical Reference: .pi/architecture/modules/plan-validation.md
Last Updated: 2026-06-19
-->

## Overview

The `plan-validation` module provides the self-correcting plan→execute→verify→fix loop
for template reliability. When a template execution fails (compile error, test failure,
quality gate unsatisfied), the validation loop parses the failure, augments the planning
context with structured feedback, and re-executes — targeting only the LLM-generated
content steps.

**Core Rule:** Deterministic steps are never retried; only `llm_generate` nodes are
retried with augmented context.

## Startup Sequence

### Dependencies

| Dependency | Required | Description |
|------------|----------|-------------|
| PlanningPipelineService | Yes | Generates plans from user intent |
| QualityGateEvaluationService | Yes | Evaluates template output quality |
| ContextAugmenter | Yes | Formats failure context for LLM retry |
| FailureParserService | No | Parses compiler/test output (optional fallback) |
| ExecutionEngine | No | Executes templates (required for full loop) |
| ValidationReportRepository | No | Persists validation reports |
| ValidatedTemplateRepository | No | Caches validated templates |

### Initialization

1. Create `ValidationLoopConfig` (default or custom)
2. Create `ValidationLoopConfigBuilder` for custom configs
3. Create `ValidationLoopImpl` with config and quality gate service
4. Optionally wrap in `ValidationReportRepository` for persistence

```rust
use plan_validation::domain::loop_config::ValidationLoopConfig;
use plan_validation::application::loop_impl::ValidationLoopImpl;

let config = ValidationLoopConfig::default();
let quality_gate = Arc::new(MyQualityGateService::new());
let validator = ValidationLoopImpl::new(config, quality_gate);
```

### Configuration Reference

| Field | Default | Description |
|-------|---------|-------------|
| `max_iterations` | 3 | One initial attempt + up to 2 retries |
| `required_quality` | Package | Minimum quality level for success |
| `max_cumulative_tokens` | 50,000 | Budget for all iterations combined |
| `cache_successful_templates` | true | Cache validated templates for replay |

Presets available via `ValidationLoopConfigPresets`:
- `development()` — 5 iterations, Workspace quality, 100K tokens, no caching
- `production()` — Defaults (3 iterations, Package, 50K tokens, caching)
- `testing()` — 2 iterations, TargetedTests, 10K tokens, no caching

## Graceful Shutdown

The validation loop cooperates with the cancellation module:

1. **Check cancellation signals** — Long-running LLM calls should check for cancellation
2. **Preserve state** — `ValidationState` is maintained for graceful resumption
3. **Roll back budget** — Budget reservations are rolled back on cancellation
4. **Return partial report** — Even on cancellation, a partial `ValidationReport` is returned

```rust
// The validate() method returns early on cancellation
match validator.validate(input).await {
    Ok(output) => match output.outcome {
        ValidationOutcome::Validated => { /* success */ }
        ValidationOutcome::Failed => { /* retries exhausted */ }
        ValidationOutcome::BudgetExhausted => { /* budget exceeded */ }
    },
    Err(ValidationLoopError::Cancelled { reason }) => {
        // Graceful shutdown: log reason, clean up
    }
}
```

## Common Failure Modes and Recovery

| Failure Mode | Symptom | Recovery |
|-------------|---------|----------|
| Budget exhausted | `ValidationOutcome::BudgetExhausted` | Increase `max_cumulative_tokens` or reduce iterations |
| All retries failed | `ValidationOutcome::Failed` | Check failure history; template may need manual correction |
| Repeated identical failures | `ContextAugmenter` detects repeats | LLM not learning from feedback; escalate to human |
| Planning pipeline error | `ValidationLoopError::PlanningError` | Check LLM budget, API keys, template registry |
| Execution error | `ValidationLoopError::ExecutionError` | Check DAG engine connectivity, tool permissions |
| Repository failure | `ValidationLoopError::RepositoryError` | Check storage backend availability |

## Monitoring

### Key Metrics
- **Validation iterations**: Count of iterations per validation session
- **Validation success rate**: % of validations that succeed on first attempt vs. retry
- **Cumulative token usage**: Total LLM tokens consumed by validation loop
- **Selective retry efficiency**: % of nodes skipped (deterministic cache hits)
- **Total duration**: Wall-clock time for full validation loop

### Logging
- All validation loop events emitted via `ValidationEvent` enum
- Structured logging with `execution_id` for correlation
- Per-iteration failure details captured in `ValidationIterationReport`

## Health Checks

The module exposes a `/health` integration that should verify:
1. `ValidationLoopConfig` loads successfully
2. `QualityGateEvaluationService` is responsive
3. LLM budget is available for planning pipeline

## See Also
- [Architecture Module](../.pi/architecture/modules/plan-validation.md)
- [DR Plan](./dr-plan-plan-validation.md)
- [Planning Pipeline Runbook](./runbook-planning-pipeline.md)
