# Runbook: Scored Evaluation Module

## Overview

The Scored Evaluation module provides quality scoring of generated artifacts via pluggable backends. It adds a `scored_evaluation` DAG node type that sends artifacts to scoring backends (MCP, HTTP, Local) and feeds results into the Policy Engine for merge gating.

## Startup Sequence

1. **Module Registration**: `pub mod scored_evaluation;` in `src/lib.rs` — module is available at compile time.
2. **Configuration Loading**: Backend configs loaded from `.rigorix/scored_evaluation.toml` (or environment variables).
3. **Backend Registration**: Each configured backend is instantiated and registered in the `ScoredEvaluationServiceImpl` backend registry.
4. **Health Check**: `ScoredEvaluationServiceImpl` performs pre-flight health checks on all registered backends.
5. **Repository Initialization**: `LocalEvaluationRepository` creates the base directory `.rigorix/evaluations/` on first use.

## Dependencies

| Dependency | Type | Failure Impact |
|------------|------|----------------|
| MCP scoring server (e.g., RuntimeAI) | External | Evaluation fails; falls back to retry policy |
| HTTP scoring endpoint | External | Evaluation fails; falls back to retry policy |
| Local scoring script | Filesystem | Evaluation fails if script not found |
| Filesystem write access | Infrastructure | Cannot persist evaluation results |
| EventBus | Internal | Events not emitted (non-fatal) |
| Audit system | Internal | Scoring results not embedded in audit envelopes (non-fatal) |

## Configuration Reference

See `.pi/architecture/modules/scored-evaluation.md#configuration` for full config schema.

Key settings:
```toml
[scored_evaluation]
default_backend = "runtimeai"

[scored_evaluation.backends.runtimeai]
type = "mcp"
timeout_ms = 30_000

[scored_evaluation.defaults]
threshold = 80
on_failure = "flag_for_review"
```

## Common Failure Modes

### BackendNotFound
- **Cause**: The configured backend name doesn't match any registered backend.
- **Recovery**: Check config file for typos. Verify backend is registered in `ScoredEvaluationServiceImpl`.
- **Is Retriable**: No — misconfiguration.

### BackendError
- **Cause**: The backend returned an error during evaluation.
- **Recovery**: Check backend logs. Verify network connectivity to external servers.
- **Is Retriable**: Yes — retries with exponential backoff (200ms, 400ms, 800ms).

### BackendUnavailable
- **Cause**: Health check failed or connection refused.
- **Recovery**: Verify backend service is running. Check network/firewall.
- **Is Retriable**: Yes.

### Timeout
- **Cause**: Backend did not respond within configured timeout.
- **Recovery**: Increase `timeout_ms` in config. Check backend performance.
- **Is Retriable**: Yes — retries with backoff.

### InvalidRubric / InvalidArtifact
- **Cause**: Rubric or artifact JSON is malformed.
- **Recovery**: Fix the template or DAG definition that produced the invalid input.
- **Is Retriable**: No — fail fast.

### ScriptNotFound (LocalBackend)
- **Cause**: Local scoring script path is invalid.
- **Recovery**: Verify the script path in config. Check file permissions.
- **Is Retriable**: No — misconfiguration.

## Graceful Shutdown

The Scored Evaluation module has no long-lived connections. Evaluations are request-response. If the application is shutting down:
1. In-flight evaluations are allowed to complete (up to `timeout_ms`).
2. No new evaluations are accepted.
3. Partial results in the repository are preserved.

## Monitoring

### Key Metrics
- `scored_evaluation.evaluations.total` — Total evaluations attempted
- `scored_evaluation.evaluations.success` — Successful evaluations
- `scored_evaluation.evaluations.failed` — Failed evaluations
- `scored_evaluation.evaluations.duration_ms` — Evaluation latency

### Logging
- `[ScoredEvaluation] Started: node=..., backend=...`
- `[ScoredEvaluation] Completed: node=..., passed=..., dimensions=...`
- `[ScoredEvaluation] Failed: node=..., error=...`

## Troubleshooting

| Symptom | Check | Resolution |
|---------|-------|------------|
| Evaluations always fail | Backend health | Run `health_check()` manually |
| Slow evaluations | Backend latency | Increase `timeout_ms` |
| Missing scoring results | Repository path | Check `.rigorix/evaluations/` exists |
| Policy conditions not triggering | LaneContext scoring_scores | Verify scores are populated in context |
