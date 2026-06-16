# rigorix CLI — Operations Runbook

> **Canonical:** `.pi/architecture/modules/cli-boundary.md`
> **Last updated:** 2026-06-16

## Overview

The `rigorix` CLI is a Rust binary crate that wraps `rigorix-engine` to provide
template-driven DAG execution. It is ephemeral per ADR-007 — each invocation
starts, does work, and exits.

## Startup Sequence

1. **Argument parsing** — clap parses CLI args into `CliCommand` enum
2. **Config loading** — `CliConfigLoaderImpl` merges CLI flags → env vars →
   `rigorix.toml` → engine defaults
3. **Tracing initialization** — `init_tracing()` sets up tracing-subscriber
   with pretty or JSON output
4. **Signal handler** — `SignalHandlerImpl` installs SIGINT handler with
   2-second double-press detection
5. **Command dispatch** — `main.rs` routes to the appropriate command handler
6. **Output formatting** — `LogFormatterImpl` renders output as pretty/JSON/quiet

## Configuration Reference

| Setting | Flag | Env Var | Config File | Default |
|---------|------|---------|-------------|---------|
| Output format | `--format` | `RIGORIX_FORMAT` | `cli.output_format` | `pretty` |
| Log level | `--log-level` | `RIGORIX_LOG` | `cli.log_level` | `info` |
| Log format | `--log-format` | — | `cli.log_format` | `pretty` |
| Color mode | `--color` | `RIGORIX_COLOR` | `cli.color` | `auto` |
| TUI enabled | `--tui`/`--no-tui` | `RIGORIX_TUI_ENABLED` | `cli.tui_enabled` | `true` |
| Config path | `--config` | `RIGORIX_CONFIG` | — | `./rigorix.toml` |
| API key | — | `RIGORIX_API_KEY` | `cli.api_key` | — |

## Graceful Shutdown

### Normal Exit
- All commands exit with code 0 on success
- Output is flushed before exit

### Cancellation (Ctrl+C — Single Press)
1. SIGINT received by `SignalHandlerImpl`
2. `ShutdownLevel::Graceful` sent to orchestrator
3. Running task finishes naturally
4. No new tasks started
5. Exit code: 130

### Force Kill (Ctrl+C — Double Press within 2s)
1. Second SIGINT received within 2-second window
2. `ShutdownLevel::Immediate` sent
3. All in-flight work aborted via `JoinSet::abort()`
4. Exit code: 137

### Crash Recovery
- State is persisted via atomic write-rename (engine handles this)
- On restart, stale `.rigorix/state/` files can be detected and recovered
- Run `rigorix history` to list past sessions

## Common Failure Modes

### 1. Config file not found
- **Symptom:** `Error: No configuration found`
- **Resolution:** Run `rigorix init` or specify `--config <path>`

### 2. Missing API key
- **Symptom:** `Error: Missing required configuration: api_key`
- **Resolution:** Set `RIGORIX_API_KEY` env var or add to `rigorix.toml`

### 3. Invalid config file
- **Symptom:** `Error: Failed to parse configuration`
- **Resolution:** Check TOML syntax; run `toml2json rigorix.toml` for validation

### 4. CLI hangs or crashes
- **Symptom:** Process doesn't respond
- **Resolution:** Double Ctrl+C for immediate abort; check logs with `RIGORIX_LOG=debug`

### 5. Engine returns error
- **Symptom:** `Error: Engine error: ...`
- **Resolution:** Check engine logs; verify engine version compatibility

## Observability

### Logging
- Structured logging via `tracing` crate
- Log format: pretty (default) or JSON (`--log-format json`)
- Log level: controlled by `RIGORIX_LOG` env var or `--log-level`

### Tracing
- Distributed tracing contexts propagated through engine
- Correlation IDs for execution sessions (UUID v4)

### Metrics
- Engine exposes Prometheus metrics (see engine runbook)
- CLI itself does not expose metrics (ephemeral process)

## Health Checks

The CLI binary itself does not expose a health endpoint. For process-level health:
- Exit code 0 = success
- Exit code non-zero = failure (see error message)
- Timeout handling: use `timeout` command for CI/CD integration

## Dependencies

| Dependency | Version | Purpose |
|------------|---------|---------|
| `rigorix-engine` | workspace | Core execution engine |
| `clap` | 4 | Argument parsing |
| `tokio` | 1 | Async runtime |
| `tracing` | 0.1 | Structured logging |
| `serde` | 1 | Serialization |
| `ratatui` | 0.29 | Terminal UI (optional) |
| `crossterm` | 0.28 | Terminal control |
