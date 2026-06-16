# CLI Boundary — Runbook

## Startup Sequence

1. **Tracing Init**: `cli_boundary::tracing::init_tracing()` — reads `RIGORIX_LOG` env var (default: `info`)
2. **Config Load**: `cli_boundary::config::load_config()` — merges `rigorix.toml` + `RIGORIX_*` env + defaults
3. **Signal Handler**: `cli_boundary::signal::install_signal_handler()` — installs Ctrl+C/SIGTERM handlers
4. **Parse Args**: `cli_boundary::cli::parse_args()` — resolves `CliCommand` from argv
5. **Dispatch**: `cli_boundary::dispatch::dispatch()` — routes command to appropriate handler

## Shutdown Sequence

| Trigger | Behaviour |
|---------|-----------|
| Single Ctrl+C | Graceful: finish in-flight node, exit 130 |
| Double Ctrl+C (within 2s) | Immediate abort, exit 137 |
| SIGTERM (Unix) | Immediate abort, exit 137 |
| Natural completion | Exit 0 |

## Configuration Sources (Priority)

| Priority | Source | Example |
|----------|--------|---------|
| 1 (highest) | CLI flags | `--max-llm-calls 100` |
| 2 | Env vars | `RIGORIX_ORCHESTRATOR_MAX_PARALLEL_TASKS=8` |
| 3 | `rigorix.toml` (CWD) | Project-level config |
| 4 | `~/.rigorix/config.toml` | User-level config |
| 5 (lowest) | Engine defaults | Compiled-in defaults |

## Common Failure Modes

| Symptom | Likely Cause | Resolution |
|---------|-------------|------------|
| Exit code 2 | Bad config file | Run `rigorix config validate` |
| Exit code 3 | Invalid arguments | Run `rigorix --help` |
| Exit code 1 | Engine error | Check logs with `RIGORIX_LOG=debug` |
| Hanging | No Ctrl+C handler | Send SIGTERM (kill -15) |
