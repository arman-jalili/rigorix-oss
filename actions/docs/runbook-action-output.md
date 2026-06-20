# Action Output Runbook

## Overview

The Action Output module formats `rigorix-engine` execution results as GitHub
Actions-native outputs — annotations, step summaries, and output variables.
It is the final module in the action pipeline, called after engine dispatch
completes to present results to the user in the Actions UI.

## Components

| Component | File | Purpose |
|-----------|------|---------|
| `OutputFormatterImpl` | `application/output_formatter_impl.rs` | Top-level orchestrator for all output channels |
| `AnnotationWriterImpl` | `application/annotation_writer_impl.rs` | Emits workflow annotations (`::error`, `::warning`, `::notice`) |
| `StepSummaryWriterImpl` | `application/step_summary_writer_impl.rs` | Writes markdown step summaries to `$GITHUB_STEP_SUMMARY` |

### Planned Components
| Component | File | Purpose |
|-----------|------|---------|
| `OutputVariableWriterImpl` | `application/` *(pending)* | Sets `$GITHUB_OUTPUT` variables |
| `PrCommentWriterImpl` | `application/` *(pending)* | Posts PR comments via GitHub API |
| `OutputRepositoryImpl` | `infrastructure/` *(pending)* | Filesystem I/O for output files |
| `EnvRepositoryImpl` | `infrastructure/` *(pending)* | Environment variable access |
| `GitHubApiClientImpl` | `infrastructure/` *(pending)* | GitHub REST API client |

## Startup Sequence

1. **Module registration** — `action_output` module is registered in `actions/src/lib.rs`
2. **Engine execution completes** — called by `action-entrypoint` after dispatch
3. **`OutputFormatterImpl::write_run_output()`** — formats and writes all outputs:
   a. Formats `ExecutionContext` into `StepSummary` markdown
   b. Writes summary to `$GITHUB_STEP_SUMMARY`
   c. Sets output variables to `$GITHUB_OUTPUT`
   d. (optional) Posts PR comment via GitHub API

## Dependencies

- **Stdout** — workflow annotations are emitted via stdout (parsed by GitHub Actions runner)
- **Filesystem** — `$GITHUB_STEP_SUMMARY` and `$GITHUB_OUTPUT` files must be accessible
- **rigorix-engine types** — `ExecutionContext`, `WorkflowAnnotation` domain types
- **GitHub API** — (optional) for posting PR comments, requires `GITHUB_TOKEN`

## Graceful Shutdown

The Action Output module has no long-lived state. Shutdown is immediate:
- No connections to close
- No caches to flush
- No background tasks
- In-flight writes to output files complete before process exit

## Common Failure Modes

| Failure Mode | Cause | Recovery |
|-------------|-------|----------|
| `MissingEnv("GITHUB_STEP_SUMMARY")` | Not running in GitHub Actions | Running locally — check non-CI fallback |
| `WriteError` | Filesystem full or permissions | Check disk space, file permissions |
| `FormatError` | Malformed execution context | Check engine output types are valid |
| `MissingToken` | GitHub token not available | Set `GITHUB_TOKEN` secret in workflow |
| `MissingPrContext` | Not running in a PR | Output falls back to summary + annotations only |
| `VariableTooLong` | Variable value > 10KB | Truncate large values in caller |

## Configuration Reference

All configuration is derived from the execution context — no direct inputs.

### Environment Variables

| Variable | Required | Purpose |
|----------|----------|---------|
| `GITHUB_STEP_SUMMARY` | No (CI) | Path to step summary file (set by GitHub Actions) |
| `GITHUB_OUTPUT` | No (CI) | Path to output variables file (set by GitHub Actions) |
| `GITHUB_TOKEN` | No | GitHub API token for PR comments |
| `GITHUB_REPOSITORY` | No | Repository owner/name for API calls |

### Output Variables (set for downstream steps)

| Variable | Type | Description |
|----------|------|-------------|
| `execution_id` | UUID | Execution identifier |
| `status` | string | `completed`, `failed`, `partial_failure` |
| `iterations` | u32 | Validation loop iterations |
| `template_id` | string | Used template ID |
| `quality_level` | string | Achieved quality level |
| `failure_count` | u32 | Number of failures |
| `cumulative_tokens` | u64 | Total LLM tokens |
| `duration_ms` | u64 | Execution duration |

## Observability

### Logging
- Module uses `tracing` for structured logging
- Key events: `OutputWritten`, `StepSummaryWritten`, `AnnotationEmitted`
- Error events include structured context for debugging
- Log level: `info` for normal operations, `warn` for recoverable issues, `error` for failures

### Tracing
- Span: `action_output` — wraps all output operations
- Child spans: `write_run_output`, `write_validation_failure`, `format_summary`
- Correlation ID: execution UUID propagated through spans

### Metrics
The module is stateless and short-lived. Key observability points:
- Annotation count per execution
- Summary bytes written
- Output variable count
- Execution status distribution (success/failure)

## Testing

```bash
# All unit tests (34 total)
cargo test --lib -p rigorix-actions -- action_output

# Proofing scripts
bash actions/.pi/scripts/ci/stage_action-output_proofing.sh
```

## Related Documents

- [Architecture doc](../.pi/architecture/modules/action-output.md)
- [DR Plan](./dr-plan-action-output.md)
- [CI Proofing Scripts](../.pi/scripts/ci/check_action-output_contracts.sh)
- [Coverage Checker](../.pi/scripts/ci/check_action-output_coverage.sh)
