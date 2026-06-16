# CLI Boundary

## Module Status

**Status:** Implemented — Phase 1 complete
**Last reviewed:** 2026-06-16
**Source session:** 71e2b81a-a7a1-48ee-ab8f-56284bbec92d

## Description

User-facing command dispatch, TUI rendering, argument parsing, and process lifecycle. The outermost shell that wires engine capabilities to the terminal.

The CLI is the entry point for all user interaction. It parses commands, loads configuration, instantiates the engine orchestrator, runs execution sessions, and renders output (TUI or JSON).

## Components

| Component | File | Module | Purpose |
|-----------|------|--------|---------|
| CommandParser | `cli/src/cli_boundary/interfaces/cli/mod.rs` | cli_boundary | Parses CLI args into CliCommand enum (Run, Plan, Init, Generate, Audit, History, Logs, Template) |
| CliOrchestrator | `cli/src/main.rs` (dispatch) | (entry point) | Top-level orchestrator: wires CommandParser → Config → Engine → output rendering |
| ExecutionSession (trait) | `cli/src/cli_boundary/application/service.rs` | cli_boundary | Manages a single execution lifecycle: load config → plan → execute → render output |
| CliOrchestratorFactory (trait) | `cli/src/cli_boundary/application/factory.rs` | cli_boundary | Factory for constructing orchestrator instances |
| TuiRenderer (trait) | `cli/src/cli_boundary/tui/mod.rs` | cli_boundary | ratatui-based terminal UI: subscribes to EventBus, renders live node graph, budget bars, status |
| LogFormatter (trait) | `cli/src/cli_boundary/infrastructure/output.rs` | cli_boundary | Formats output as human-readable or JSON for CI/CD integration |
| LogFormatterImpl | `cli/src/cli_boundary/infrastructure/output_impl.rs` | cli_boundary | Pretty, JSON, and quiet output formatter implementation |

## Domain Events

| Event | Description | Triggered By |
|-------|-------------|-------------|
| CommandDispatched | A CLI command was parsed and dispatched | CommandParser |
| SessionStarted | An execution session began | ExecutionSession |
| SessionCompleted | Execution session finished (success, failure, or cancelled) | ExecutionSession |

## Ubiquitous Language

| Term | Definition |
|------|-----------|
| CliCommand | Parsed CLI command variant: Run, Plan, Init, Generate, Audit, History, Logs |
| ExecutionSession | A single CLI-managed execution run linking engine execution_id to CLI session metadata |
| TuiRenderer | ratatui-based terminal UI rendering engine execution events in real-time |
| LogFormatter | Formats engine output as human-readable or JSON |

## Implementation Details

### Architecture Layers

Each bounded context is a self-contained module with Clean Architecture layers:

```
cli/src/
├── cli_boundary/     # Shared CLI types — command dispatch, output, TUI
│   ├── domain/       # CliError enum (12 variants), CliEvent enum (6 event types)
│   │   ├── error.rs
│   │   └── event/
│   ├── application/  # CliOrchestrator, ExecutionSession traits, DTOs
│   │   ├── service.rs
│   │   ├── factory.rs
│   │   └── dto/      # 20+ input/output DTOs
│   ├── infrastructure/  # LogFormatter trait + impl
│   │   ├── output.rs
│   │   ├── output_impl.rs
│   │   └── repository/  # Reserved for future persistence
│   ├── interfaces/   # Clap CLI command definitions
│   │   └── cli/
│   ├── tui/          # TUI renderer trait (ratatui)
│   └── tests.rs      # 35+ contract tests
├── configuration/    # Multi-source config loading
│   ├── domain/       # CliConfig value object
│   │   └── config.rs
│   └── infrastructure/  # CliConfigLoader trait + CliConfigLoaderImpl
│       ├── config.rs
│       └── config_impl.rs
├── observability/    # Tracing, health checks, event schemas
│   ├── domain/event/ # ObservabilityEvent payload schemas
│   │   └── observability.rs
│   └── infrastructure/  # TracingInitializer trait + tracing impl
│       ├── observability.rs
│       └── tracing.rs
├── cancellation/     # Signal handler for Ctrl+C
│   └── infrastructure/  # SignalHandler trait + SignalHandlerImpl
│       ├── signal.rs
│       └── signal_impl.rs
├── main.rs           # Binary entry point with full command dispatch
└── lib.rs            # Library root
```

### Config Loading Priority
1. CLI flag overrides (highest)
2. Environment variables (`RIGORIX_*`)
3. `rigorix.toml` config file
4. Engine defaults (lowest)

### Signal Handling
- Single Ctrl+C → `ShutdownLevel::Graceful` (exit code 130)
- Double Ctrl+C within 2s → `ShutdownLevel::Immediate` (exit code 137)

## Proofing & CI

### Proofing Scripts (`.pi/scripts/ci/`)
| Script | Module | What it Checks | |
|--------|--------|---------------|-|
| `check_cli_contracts.sh` | cli_boundary | Each trait has a concrete impl | 7/7 pass |
| `check_cli_coverage.sh` | cli_boundary | Test counts per module | 30+ total |
| `stage_cli_proofing.sh` | cli_boundary | CI stage wrapper | 3/3 pass |
| `check_config_contracts.sh` | configuration | 17 config contract checks | 17/17 pass |
| `check_config_coverage.sh` | configuration | Config module coverage thresholds | 3/3 pass |
| `stage_config_proofing.sh` | configuration | CI stage wrapper | 3/3 pass |
| `check_observability_contracts.sh` | observability | 15 observability contract checks | 15/15 pass |
| `check_observability_coverage.sh` | observability | Observability module coverage | 3/3 pass |
| `stage_observability_proofing.sh` | observability | CI stage wrapper | 3/3 pass |

### CI Stages
- Stage 11 — `cli_proofing` — runs on every PR
- Stage 12 — `config_proofing` — runs on every PR
- Stage 13 — `observability_proofing` — runs on every PR

## Dependencies

- Depends on: `rigorix-engine` crate (all engine modules via a single orchestrator facade)
- Depends on: `Configuration` (loads engine config from rigorix.toml)
- Depends on: `Cancellation` (forwards cancellation signals to engine)
- Depends on: `State Persistence` (reads execution history for `rigorix history`)
- Depends on: `Audit` (queries audit envelopes for `rigorix audit`)
- Depends on: `Template Generation` (exposes `rigorix generate` command)
- Depends on: `Planning Pipeline` (exposes `rigorix plan` command)

## Key Files

| File | Purpose |
|------|---------|
| `cli/Cargo.toml` | CLI crate manifest with rigorix-engine dependency |
| File | Module | Purpose |
|------|--------|---------|
| `cli/src/main.rs` | (entry point) | Binary entry point with command dispatch |
| `cli/src/lib.rs` | (root) | Library root — re-exports all modules |
| `cli/src/cli_boundary/interfaces/cli/mod.rs` | cli_boundary | CliCommand enum and argument parsing (clap) |
| `cli/src/cli_boundary/application/service.rs` | cli_boundary | CliOrchestrator + ExecutionSession traits |
| `cli/src/cli_boundary/application/factory.rs` | cli_boundary | Factory interfaces |
| `cli/src/cli_boundary/application/dto/mod.rs` | cli_boundary | 20+ input/output DTOs |
| `cli/src/cli_boundary/domain/error.rs` | cli_boundary | CliError enum (12 variants) |
| `cli/src/cli_boundary/domain/event/mod.rs` | cli_boundary | CliEvent enum (6 event types) |
| `cli/src/cli_boundary/infrastructure/output.rs` | cli_boundary | LogFormatter trait |
| `cli/src/cli_boundary/infrastructure/output_impl.rs` | cli_boundary | LogFormatterImpl — pretty, JSON, quiet |
| `cli/src/cli_boundary/tui/mod.rs` | cli_boundary | ratatui terminal UI trait |
| `cli/src/cli_boundary/tests.rs` | cli_boundary | 35+ contract tests |
| `cli/src/configuration/infrastructure/config_impl.rs` | configuration | Config loader implementation |
| `cli/src/cancellation/infrastructure/signal_impl.rs` | cancellation | Signal handler implementation |
| `cli/src/observability/infrastructure/tracing.rs` | observability | Tracing initialization |
| `cli/docs/runbook.md` | docs | Operations runbook |
| `cli/docs/dr-plan.md` | docs | Disaster recovery plan |

## ADRs

| ADR | Title | Status |
|-----|-------|--------|
| ADR-001 | Domain-Driven Design with Bounded Contexts | Implemented |
| ADR-002 | CLI/Engine Split | Implemented |
| ADR-003 | Ratatui TUI | Planned (Phase 5) |
| ADR-004 | TOML Template Format | Contract defined |
| ADR-005 | EventBus Pub-Sub | Contract defined |
| ADR-007 | Ephemeral CLI | Implemented |
| ADR-009 | Claude LLM Provider | Contract defined |
| ADR-010 | Persist Generated Templates | Contract defined |
