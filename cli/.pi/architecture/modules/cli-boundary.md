# CLI Boundary

## Module Status

**Status:** Planned — first module to implement
**Last reviewed:** 2026-06-16
**Source session:** 71e2b81a-a7a1-48ee-ab8f-56284bbec92d

## Description

User-facing command dispatch, TUI rendering, argument parsing, and process lifecycle. The outermost shell that wires engine capabilities to the terminal.

The CLI is the entry point for all user interaction. It parses commands, loads configuration, instantiates the engine orchestrator, runs execution sessions, and renders output (TUI or JSON).

## Components

| Component | File (planned) | Purpose |
|-----------|---------------|---------|
| CommandParser | `cli/src/command.rs` | Parses CLI args into CliCommand enum (Run, Plan, Init, Generate, Audit, History, Logs, Template) |
| CliConfigLoader | `cli/src/config.rs` | Loads and merges CLI-specific config (output format, TUI enabled) with engine Config |
| ExecutionSession | `cli/src/session.rs` | Manages a single execution lifecycle: load config → plan → execute → render output |
| TuiRenderer | `cli/src/tui/mod.rs` | ratatui-based terminal UI: subscribes to EventBus, renders live node graph, budget bars, status |
| LogFormatter | `cli/src/output.rs` | Formats output as human-readable or JSON for CI/CD integration |
| SignalHandler | `cli/src/signal.rs` | Captures Ctrl+C (graceful) and double-Ctrl+C (immediate) for cancellation |
| CliOrchestrator | `cli/src/orchestrator.rs` | Top-level orchestrator: wires CommandParser → Config → Engine → output rendering |

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

## Dependencies

- Depends on: `rigorix-engine` crate (all engine modules via a single orchestrator facade)
- Depends on: `Configuration` (loads engine config from rigorix.toml)
- Depends on: `Event System` (subscribes to ExecutionEvent stream for TUI)
- Depends on: `Cancellation` (forwards cancellation signals to engine)
- Depends on: `State Persistence` (reads execution history for `rigorix history`)
- Depends on: `Audit` (queries audit envelopes for `rigorix audit`)
- Depends on: `Template Generation` (exposes `rigorix generate` command)
- Depends on: `Planning Pipeline` (exposes `rigorix plan` command)

## Key Files

| File | Purpose |
|------|---------|
| `cli/src/main.rs` | Binary entry point |
| `cli/src/command.rs` | CliCommand enum and argument parsing (clap) |
| `cli/src/orchestrator.rs` | Top-level CLI orchestrator |
| `cli/src/session.rs` | Execution session management |
| `cli/src/tui/mod.rs` | ratatui terminal UI |
| `cli/src/output.rs` | Output formatting (human/JSON) |
| `cli/src/signal.rs` | Ctrl+C signal handling |
| `cli/Cargo.toml` | CLI crate manifest with rigorix-engine dependency |

## ADRs

| ADR | Title | Status |
|-----|-------|--------|
| ADR-001 | Domain-Driven Design with Bounded Contexts | Proposed |
