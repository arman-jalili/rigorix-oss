# CLI Boundary

## Module Status

**Status:** Implemented — Phase 1 complete
**Last reviewed:** 2026-06-16
**Source session:** 71e2b81a-a7a1-48ee-ab8f-56284bbec92d

## Description

User-facing command dispatch, TUI rendering, argument parsing, and process lifecycle. The outermost shell that wires engine capabilities to the terminal.

The CLI is the entry point for all user interaction. It parses commands, loads configuration, instantiates the engine orchestrator, runs execution sessions, and renders output (TUI or JSON).

## Components

| Component | File (implemented) | Purpose |
|-----------|---------------|---------|
| CommandParser | `cli/src/interfaces/cli/mod.rs` | Parses CLI args into CliCommand enum (Run, Plan, Init, Generate, Audit, History, Logs, Template) |
| CliConfigLoader | `cli/src/infrastructure/config_impl.rs` | Loads and merges CLI-specific config (output format, TUI enabled) with engine Config |
| CliOrchestrator | `cli/src/main.rs` (dispatch) | Top-level orchestrator: wires CommandParser → Config → Engine → output rendering |
| ExecutionSession | `cli/src/application/service.rs` (trait) | Manages a single execution lifecycle: load config → plan → execute → render output |
| TuiRenderer | `cli/src/tui/mod.rs` (trait) | ratatui-based terminal UI: subscribes to EventBus, renders live node graph, budget bars, status |
| LogFormatter | `cli/src/infrastructure/output_impl.rs` | Formats output as human-readable or JSON for CI/CD integration |
| SignalHandler | `cli/src/infrastructure/signal_impl.rs` | Captures Ctrl+C (graceful) and double-Ctrl+C (immediate) for cancellation |

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

```
cli/src/
├── domain/           # CLI-specific domain types
│   ├── config.rs     # CliConfig, OutputFormat, ColorMode, LogLevel, LogFormat
│   ├── error.rs      # CliError enum (12 variants, exit codes, retriable detection)
│   └── event/        # CliEvent enum (5 event types)
├── application/      # Service traits, DTOs, factory interfaces
│   ├── service.rs    # CliOrchestrator trait (10 commands), ExecutionSession trait
│   ├── factory.rs    # Factory interfaces
│   └── dto/          # 20+ input/output DTOs
├── infrastructure/   # I/O implementations
│   ├── config.rs     # CliConfigLoader trait
│   ├── config_impl.rs # CliConfigLoaderImpl — multi-source merging
│   ├── output.rs     # LogFormatter trait
│   ├── output_impl.rs # LogFormatterImpl — pretty, JSON, quiet
│   ├── signal.rs     # SignalHandler trait + ShutdownLevel enum
│   ├── signal_impl.rs # SignalHandlerImpl — double-press Ctrl+C
│   └── repository/   # Reserved for future persistence
├── interfaces/       # API contracts
│   └── cli/          # Clap CLI command definitions
├── tracing.rs        # Tracing initialization (pretty/JSON)
├── tui/              # TUI renderer trait (ratatui)
├── main.rs           # Binary entry point with full command dispatch
├── lib.rs            # Library root
└── tests.rs          # 35+ contract tests
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
| Script | What it Checks | Threshold |
|--------|---------------|-----------|
| `check_cli_contracts.sh` | Each trait has a concrete impl | 7/7 pass |
| `check_cli_coverage.sh` | Test counts per module | 30+ total |
| `stage_cli_proofing.sh` | CI stage wrapper | 3/3 pass |

### CI Stage
- Stage 11 in `run_hardening_stages.sh` — runs on every PR

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
| `cli/src/main.rs` | Binary entry point with command dispatch |
| `cli/src/lib.rs` | Library root with all modules |
| `cli/src/interfaces/cli/mod.rs` | CliCommand enum and argument parsing (clap) |
| `cli/src/infrastructure/config_impl.rs` | Config loader implementation |
| `cli/src/infrastructure/signal_impl.rs` | Signal handler implementation |
| `cli/src/infrastructure/output_impl.rs` | Output formatting implementation |
| `cli/src/tracing.rs` | Tracing initialization |
| `cli/src/tui/mod.rs` | ratatui terminal UI trait |
| `cli/docs/runbook.md` | Operations runbook |
| `cli/docs/dr-plan.md` | Disaster recovery plan |

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
