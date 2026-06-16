# CLI Boundary

## Status

**Status:** ✅ Architecture defined — source code implemented
**Last reviewed:** 2026-06-16

## Description

Thin binary module for command parsing, configuration, signal handling, output formatting, and dispatch. One of two peer modules under the CLI crate (`cli/`). The other is `tui/` (terminal UI).

Per ADR-002, the CLI contains zero business logic — all execution, planning, and domain logic lives in the engine crate. The engine's `OrchestratorService` provides the top-level entry point for `run`, `plan`, `cancel`, and `status`.

## Responsibilities

| Responsibility | Implementation |
|---------------|---------------|
| Command parsing | Clap argument parser → `CliCommand` enum (14 commands) |
| Config loading | Merge `rigorix.toml` + env vars + CLI flags → engine `Config` |
| Orchestrator wiring | Build orchestrator via `OrchestratorBuilder` for `run`/`plan`/`cancel`/`status` |
| Signal handling | Ctrl+C detection → forward to engine `CancellationToken` |
| Tracing init | `tracing-subscriber` with `RIGORIX_LOG` env filter |
| Output formatting | Pretty (human), JSON (CI/CD), Markdown, Quiet modes |

## Commands

### Tier 1 — Via `OrchestratorService`

| Command | Orchestrator Method | Engine Services Wired |
|---------|--------------------|-----------------------|
| `rigorix run <intent>` | `orchestrator.run(RunInput)` | PlanningPipeline → ParallelExecutionService → StateManagerService → EventBus → Audit |
| `rigorix plan <intent>` | `orchestrator.plan_only(PlanOnlyInput)` | PlanningPipeline only |
| `rigorix cancel <id>` | `orchestrator.cancel(CancelInput)` | CancellationService propagation |
| `rigorix status <id>` | `orchestrator.status()` | StateManagerService → load_state |

### Tier 2 — Via Engine Services Directly

| Command | Engine API Called |
|---------|-------------------|
| `rigorix history [--limit] [--status]` | `state_persistence::StateManagerService::list_executions()` |
| `rigorix explain <id> [--diff <id2>]` | `state_persistence::StateManagerService::load_state()` |
| `rigorix diff-plan <id1> <id2>` | `dag_engine::DagPlanningService::compare_plans()` |
| `rigorix generate <intent>` | `template_generation::TemplateGenerator::generate()` |
| `rigorix template list\|show` | `templates::TemplateEngine` |
| `rigorix audit list\|show\|diff` | `audit::AuditService` |
| `rigorix logs <session>` | `event_system::EventBus::subscribe()` |
| `rigorix config init\|show\|validate` | `configuration::ConfigService::load()/validate()` |

### Tier 3 — CLI-Only

| Command | What it does |
|---------|-------------|
| `rigorix init` | Scaffold `.rigorix/` directory + default config |
| `rigorix key` | Generate API keys |
| `rigorix` (no args) | **Primary interface** — launch interactive TUI dashboard (see tui.md) |
| `rigorix tui [--exec] [--history]` | Launch TUI with specific execution loaded |

### Shortcut Flags

| Flag | Expands To |
|------|-----------|
| `--run <intent>` | `rigorix run <intent>` |
| `--exec <id>` | `rigorix tui --exec <id>` |
| `--history` | `rigorix history` |

## Module Structure

```
cli/src/
├── main.rs                   # Binary entry point
├── lib.rs                    # Library root
│   ├── pub mod cli_boundary;
│   └── pub mod tui;
│
├── cli_boundary/             # ← This module
│   ├── cli.rs                # Clap: 14 commands + --format, -v, shortcuts
│   ├── dispatch.rs           # match command → orchestrator / engine / CLI-only
│   ├── orchestrator.rs       # Wires OrchestratorBuilder with config + repo_root
│   ├── config.rs             # Config loader (TOML + env + flags → engine Config)
│   ├── output.rs             # LogFormatter trait (Pretty, JSON, Markdown, Quiet)
│   ├── output_impl.rs        # Formatter implementations
│   ├── signal.rs             # SignalHandler (Ctrl+C → orchestrator.cancel())
│   ├── tracing.rs            # Tracing initialization
│   ├── error.rs              # CliError → exit codes (0, 1, 2, 3, 130, 137)
│   └── tests.rs              # Integration tests
│
└── tui/                      # Terminal UI (see tui.md)
    ├── ...
```

### Dispatch Logic

```
dispatch(command, config) {
    match command {
        // Via OrchestratorService:
        Run      → let orch = build_orchestrator(config);
                    let result = orch.run(RunInput { intent, config, repo_root }).await;
                    formatter.format_run(result)
        Plan     → let orch = build_orchestrator(config);
                    let result = orch.plan_only(PlanOnlyInput { intent, config }).await;
                    formatter.format_plan(result)
        Cancel   → let orch = build_orchestrator(config);
                    orch.cancel(CancelInput { execution_id }).await
        Status   → let orch = build_orchestrator(config);
                    orch.status().await

        // Via Engine Services Directly:
        History  → engine::state_persistence::list_executions(limit, status).await
        Explain  → engine::state_persistence::load_state(execution_id).await
        DiffPlan → engine::dag_engine::compare_plans(id1, id2).await
        Generate → engine::template_generation::generate(intent).await
        Template → engine::templates::list() / show(id)
        Audit    → engine::audit::list() / show()
        Logs     → engine::event_system::subscribe(session_id)
        Config   → engine::configuration::validate() / load()

        // CLI-Only:
        Init     → scaffold .rigorix/ + default rigorix.toml
        Key      → generate API key
        Tui      → launch ratatui dashboard (tui::run(args).await)
    }
}
```

## Config Loading Priority

1. CLI flag overrides (highest)
2. Environment variables (`RIGORIX_*`)
3. `rigorix.toml` config file
4. Engine defaults (lowest)

The CLI loads and merges these sources, then passes the result to `engine::configuration::ConfigService::load()`.

## Signal Handling

- Single Ctrl+C → `CancellationService::request_graceful_shutdown()` (finish in-flight node, exit 130)
- Double Ctrl+C within 2s → `CancellationService::request_immediate_abort()` (exit 137)
- SIGTERM → immediate abort (Unix only)

Signal handler is installed at startup and shared with the orchestrator via `CancellationToken`.

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | General error / engine error |
| 2 | Configuration error |
| 3 | Invalid command or arguments |
| 130 | Cancelled by user (Ctrl+C) |
| 137 | Killed / timeout |

## Output Formats

| Format | Flag | Use Case |
|--------|------|----------|
| Pretty | `--format pretty` (default) | Human-readable with Unicode symbols |
| JSON | `--format json` | CI/CD integration, scripting |
| Markdown | `--format markdown` | Documentation output |
| Quiet | `--format quiet` | Minimal output, exit codes only |

## Domain Events

| Event | Description | Trigger |
|-------|-------------|---------|
| CommandDispatched | A CLI command was parsed and dispatched | `main()` |
| SessionStarted | Execution session began | `run` or `plan` command |
| SessionCompleted | Execution finished (success, failure, cancelled) | Orchestrator returns |

## Components

### CliParser

status: planned
depends: none
**Purpose:** Clap argument parser defining 14 CLI commands, global flags (--format, -v), and shortcut flags (--run, --exec, --history). Produces `CliCommand` enum.

### Dispatcher

status: planned
depends: CliParser
**Purpose:** Match parsed `CliCommand` to the appropriate handler — orchestrator (run/plan/cancel/status), engine service (history/explain/diff-plan/generate/template/audit/logs/config), or CLI-only (init/key/tui).

### OrchestratorBuilder

status: planned
depends: ConfigLoader
**Purpose:** Build and wire `OrchestratorService` with config + repo_root. Passes shared `CancellationToken` from signal handler. Used by `run`, `plan`, `cancel`, `status` commands.

### ConfigLoader

status: planned
depends: none
**Purpose:** Merge `rigorix.toml` + environment variables (`RIGORIX_*`) + CLI flags with priority ordering. Produces engine `Config`.

### OutputFormatter

status: planned
depends: none
**Purpose:** `LogFormatter` trait with four implementations — Pretty (human-readable with Unicode), JSON (CI/CD), Markdown (docs), Quiet (exit codes only). Drives all command output.

### SignalHandler

status: planned
depends: none
**Purpose:** Ctrl+C detection with two-level cancellation (single = graceful finish in-flight node, double within 2s = immediate abort). SIGTERM handling for Unix. Shares `CancellationToken` with orchestrator.

### TracingInit

status: planned
depends: none
**Purpose:** Initialize `tracing-subscriber` with `RIGORIX_LOG` env filter. Installed at startup before any command dispatch.

### CliError

status: planned
depends: none
**Purpose:** Error type mapping engine errors to CLI exit codes (0 success, 1 general error, 2 config error, 3 invalid args, 130 cancelled, 137 killed).

## Ubiquitous Language

| Term | Definition |
|------|-----------|
| CliCommand | Parsed CLI command (14 variants) |
| CliError | CLI error type mapping engine errors to exit codes |
| LogFormatter | Formats engine output as Pretty, JSON, Markdown, or Quiet |
| OrchestratorBuilder | Wires engine's `OrchestratorService` with config and repo_root |

## Dependencies

- Depends on: `rigorix-engine` crate (orchestrator, state_persistence, dag_engine, audit, event_system, configuration, templates, template_generation)
- Depends on: `clap` (arg parsing), `tracing` + `tracing-subscriber` (logging)
- Shares `CliConfig` with `tui/` module

## ADRs

| ADR | Title | Status |
|-----|-------|--------|
| ADR-002 | CLI/Engine Split | Accepted |
| ADR-003 | Ratatui TUI | Accepted |
| ADR-007 | Ephemeral CLI — No Daemon for v1 | Accepted |

## Cross-Reference

| Engine Module | Used By CLI Command |
|---------------|---------------------|
| `orchestrator` | `run`, `plan`, `cancel`, `status` |
| `state_persistence` | `history`, `explain` |
| `dag_engine` | `diff-plan` |
| `template_generation` | `generate` |
| `templates` | `template` |
| `audit` | `audit` |
| `event_system` | `logs`, `tui` |
| `configuration` | `config`, startup |
| `cancellation` | signal handler |
