# Domain Exploration: CLI Boundary

> **Status:** Complete — architecture simplified per ADR-002

## Business Context

The **rigorix CLI** is a thin binary wrapper around the `rigorix-engine` library crate. It handles:

- User-facing command dispatch (clap argument parsing)
- Configuration loading and merging (TOML + env + flags)
- Terminal signal handling (Ctrl+C detection and forwarding)
- Tracing/logging initialization
- Output formatting (Pretty, JSON, Quiet)
- TUI rendering (ratatui, Phase 2)

**All business logic** — planning, execution, templates, budgets, enforcement, persistence, auditing — lives in the engine crate. The CLI calls engine APIs directly with no wrapper layer.

## Key Architectural Decisions

1. **Single CLI module:** `cli_boundary` — no mirror modules for engine concepts
2. **Direct engine consumption:** CLI calls `rigorix_engine::planning::...`, etc. — no `CliPlanService` traits
3. **No CLI-side DTOs/errors/events for engine concepts:** Use engine types directly
4. **No HTTP interfaces:** The CLI is a terminal binary, not an HTTP server
5. **No Repository traits:** The CLI has no persistence — engine handles all state

## Actors

| Actor | Description |
|-------|-------------|
| Developer | Runs CLI commands, monitors TUI, approves risk gates |
| CI/CD System | Invokes headless CLI with `--json` flag, parses structured output |
| LLM Provider | Called by engine during planning phase (not CLI concern) |

## CLI Commands

| Command | Engine API Called | Output |
|---------|------------------|--------|
| `rigorix run <intent>` | `planning::plan()` + `execution_engine::execute()` | TUI / JSON / Pretty |
| `rigorix plan <intent>` | `planning::plan()` | JSON / Pretty |
| `rigorix init` | Filesystem only | Pretty |
| `rigorix generate <intent>` | `template_generation::generate()` | Pretty / stdout |
| `rigorix history` | `state_persistence::list()` / `load()` | TUI / Pretty |
| `rigorix audit` | `audit::list()` / `show()` | Pretty / JSON |
| `rigorix logs <session>` | `event_system::subscribe()` | TUI / Pretty |
| `rigorix template` | `templates::list()` / `show()` | Pretty |

## Architecture Flow

```
User types command
  → clap parses args into CliCommand
  → ConfigLoader merges toml + env + flags
  → SignalHandler installs Ctrl+C listener
  → TracingInitializer starts logging
  → dispatch() calls engine API directly
  → LogFormatter renders engine result
  → main() exits with code
```
