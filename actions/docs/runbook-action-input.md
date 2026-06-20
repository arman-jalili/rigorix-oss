# Action Input Runbook

## Overview

The Action Input module parses GitHub Actions environment variables, event payloads,
and workflow inputs into typed Rust structs. It is the first module in the action
pipeline — every execution begins here.

## Components

| Component | File | Purpose |
|-----------|------|---------|
| `InputParserImpl` | `application/input_parser_impl.rs` | Parses `INPUT_*` env vars into `ActionInputs` |
| `CommentParserImpl` | `application/comment_parser_impl.rs` | Parses `/rigorix` commands from comments |
| `CiDetectorImpl` | `application/ci_detector_impl.rs` | Detects CI environment, sets permissions |
| `ConfigLoaderImpl` | `application/config_loader_impl.rs` | Merges env/CLI/YAML config sources |
| `EnvInputRepository` | `infrastructure/env_input_repository_impl.rs` | Reads env vars from `std::env` |

## Startup Sequence

1. **Module registration** — `action_input` module is registered in `actions/src/lib.rs`
2. **`ConfigLoaderImpl::load()`** — called first, merges config from all sources
3. **`InputParserImpl::parse()`** — called to read `INPUT_*` environment variables
4. **`CiDetectorImpl::detect()`** — called to determine CI/local context
5. If event is `IssueComment`: **`CommentParserImpl::parse()`** — extracts slash commands

## Dependencies

- **None** — Action Input is a standalone module. It depends only on:
  - Standard library (`std::env`, `std::fs`)
  - `serde` / `serde_json` / `serde_yaml` for parsing
  - `tokio` for async I/O
  - `uuid` for validation

## Graceful Shutdown

The Action Input module has no long-lived state. Shutdown is immediate:
- No connections to close
- No caches to flush
- No background tasks

## Common Failure Modes

| Failure Mode | Cause | Recovery |
|-------------|-------|----------|
| `MissingRequiredInput` | Required env var not set | Check workflow YAML `with:` block |
| `InvalidInputValue` | Non-numeric value for numeric field | Fix workflow YAML input value |
| `EventPayloadNotFound` | `GITHUB_EVENT_PATH` missing | Ensure running in GitHub Actions context |
| `EventPayloadParseError` | Malformed event JSON | Check GitHub API compatibility |
| `ActionYmlNotFound` | Missing `action.yml` | Add `action.yml` to repository root |
| `ActionYmlParseError` | Invalid `action.yml` YAML | Run `yamllint action.yml` |

## Configuration Reference

All inputs are passed as `INPUT_<NAME>` environment variables:

| Input | Env Var | Type | Default |
|-------|---------|------|---------|
| intent | `INPUT_INTENT` | string (opt) | None |
| mode | `INPUT_MODE` | string (opt) | `auto` |
| permission-mode | `INPUT_PERMISSION_MODE` | string (opt) | `workspace_write` (CI) / `prompt` (local) |
| policy-file | `INPUT_POLICY_FILE` | string (opt) | `.rigorix/policy.toml` |
| fail-on-violation | `INPUT_FAIL_ON_VIOLATION` | bool (opt) | `false` |
| fail-on-action-error | `INPUT_FAIL_ON_ACTION_ERROR` | bool (opt) | `false` |
| max-llm-calls | `INPUT_MAX_LLM_CALLS` | u32 (opt) | `50` |
| max-llm-tokens | `INPUT_MAX_LLM_TOKENS` | u64 (opt) | `50000` |
| max-validation-iterations | `INPUT_MAX_VALIDATION_ITERATIONS` | u32 (opt) | `3` |
| max-retries | `INPUT_MAX_RETRIES` | u32 (opt) | `3` |
| retry-delay-ms | `INPUT_RETRY_DELAY_MS` | u64 (opt) | `1000` |
| post-pr-comment | `INPUT_POST_PR_COMMENT` | bool (opt) | `true` |
| profile | `INPUT_PROFILE` | string (opt) | None |

## Metrics

The module exposes no metrics directly (it is stateless and short-lived). Downstream
consumers may track:
- Parse duration (from InputParser)
- Config resolution duration (from ConfigLoader)
- Parse warning count

## Testing

```bash
# Unit + integration tests
cargo test -p rigorix-actions

# Proofing scripts
bash actions/.pi/scripts/ci/stage_action-input_proofing.sh
```

## Related Documents

- [Architecture doc](../.pi/architecture/modules/action-input.md)
- [DR Plan](./dr-plan-action-input.md)
