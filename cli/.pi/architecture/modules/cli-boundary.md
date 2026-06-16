# CLI Boundary

## Status

**Status:** ✅ Architecture defined — source code removed pending regeneration
**Last reviewed:** 2026-06-16

## Description

User-facing command-line interface for Rigorix. A thin binary that parses commands, loads configuration, calls the `rigorix-engine` library, and renders output. Per ADR-002, the CLI contains zero business logic — all execution, planning, and domain logic lives in the engine crate.

## Responsibilities

| Responsibility | Implementation |
|---------------|---------------|
| Command parsing | Clap argument parser → `CliCommand` enum |
| Config loading | Merge `rigorix.toml` + env vars + CLI flags → engine `Config` |
| Signal handling | Ctrl+C detection → forward to engine `CancellationToken` |
| Tracing init | `tracing-subscriber` with `RIGORIX_LOG` env filter |
| Output formatting | Pretty (human), JSON (CI/CD), Quiet modes |
| TUI rendering | Ratatui — subscribes to engine EventBus (Phase 2) |

## Commands

| Command | Description | Engine Dependency |
|---------|-------------|-------------------|
| `rigorix run <intent>` | Execute a plan | `planning::PlanningPipelineService`, `execution_engine::executor` |
| `rigorix plan <intent>` | Preview a plan | `planning::PlanningPipelineService` |
| `rigorix init` | Scaffold project | Filesystem only |
| `rigorix generate <intent>` | Generate a template | `template_generation::TemplateGenerator` |
| `rigorix history` | List/show past sessions | `state_persistence::StateManager` |
| `rigorix audit` | View audit trails | `audit::AuditService` |
| `rigorix logs` | Stream execution events | `event_system::EventBus` |
| `rigorix template` | List/show templates | `templates::TemplateRegistry` |

## Architecture

```
cli/src/
├── main.rs                  # Binary entry point
├── lib.rs                   # Library root (re-exports cli_boundary)
└── cli_boundary/
    ├── cli.rs               # Clap command definitions (CliCommand enum)
    ├── dispatch.rs           # Main dispatch: command → engine → format
    ├── config.rs             # Config loader (TOML + env + flags)
    ├── config_impl.rs        # Config loader implementation
    ├── output.rs             # LogFormatter trait
    ├── output_impl.rs        # Pretty/JSON/Quiet formatters
    ├── signal.rs             # SignalHandler (Ctrl+C)
    ├── tracing.rs            # Tracing initialization
    ├── tui.rs                # Ratatui renderer (Phase 2)
    ├── error.rs              # CliError → exit codes
    └── tests.rs              # Integration tests
```

## Config Loading Priority

1. CLI flag overrides (highest)
2. Environment variables (`RIGORIX_*`)
3. `rigorix.toml` config file
4. Engine defaults (lowest)

The CLI loads and merges these sources, then passes the result to `engine::configuration::ConfigService::load()`.

## Signal Handling

- Single Ctrl+C → `ShutdownLevel::Graceful` (finish in-flight node, exit 130)
- Double Ctrl+C within 2s → `ShutdownLevel::Immediate` (abort all, exit 137)

Signal handler forwards to `engine::cancellation::CancellationService`.

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | General error / engine error |
| 2 | Configuration error |
| 3 | Invalid command |
| 130 | Cancelled (Ctrl+C) |
| 137 | Killed / timeout |

## Domain Events

| Event | Description | Trigger |
|-------|-------------|---------|
| CommandDispatched | A CLI command was parsed and dispatched | `dispatch()` |
| SessionStarted | Execution session began | `run` command |
| SessionCompleted | Execution session finished | Engine returns result |

## Ubiquitous Language

| Term | Definition |
|------|-----------|
| CliCommand | Parsed CLI command (Run, Plan, Init, Generate, History, Logs, Audit, Template) |
| CliError | CLI error type mapping engine errors to exit codes |
| LogFormatter | Formats engine output as Pretty, JSON, or Quiet |
| TuiRenderer | Ratatui-based terminal UI |

## Dependencies

- Depends on: `rigorix-engine` crate (all engine modules via library API)
- Depends on: `clap` (arg parsing), `ratatui` (TUI), `tracing` (logging)

## ADRs

| ADR | Title | Status |
|-----|-------|--------|
| ADR-002 | CLI/Engine Split | Accepted |
| ADR-003 | Ratatui TUI | Accepted |
| ADR-007 | Ephemeral CLI — No Daemon for v1 | Accepted |
