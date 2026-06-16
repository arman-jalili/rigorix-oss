# Project Context — rigorix-cli

## Project Overview

- **Name:** rigorix-cli
- **Version:** 0.1.0
- **Language:** Rust
- **Type:** binary (thin wrapper around rigorix-engine library)
- **Repository:** arman-jalili/rigorix-oss

## Core Architecture

The CLI is a **thin binary** with one module (`cli_boundary`). All business logic lives in the `rigorix-engine` library crate.

```
cli/src/
├── main.rs           # Entry point: parse → load → dispatch → format → exit
├── lib.rs            # Re-exports cli_boundary
└── cli_boundary/
    ├── cli.rs        # Clap command definitions
    ├── dispatch.rs   # Command → engine API → format
    ├── config.rs     # Config loading (TOML + env + flags)
    ├── config_impl.rs
    ├── output.rs     # LogFormatter trait
    ├── output_impl.rs # Pretty/JSON/Quiet formatters
    ├── signal.rs     # SignalHandler (Ctrl+C)
    ├── tracing.rs    # Tracing init
    ├── tui.rs        # Ratatui renderer (Phase 2)
    ├── error.rs      # CliError → exit codes
    └── tests.rs
```

## Architecture Principles

1. **CLI calls engine directly** — no wrapper traits, no mirror DTOs, no parallel domain layers
2. **No business logic in CLI** — all planning, execution, templates, budgets, enforcement lives in engine
3. **Single module** — `cli_boundary` is the only CLI module
4. **Exit codes:** 0=success, 1=error, 2=config error, 3=invalid command, 130=cancelled, 137=killed

## Key Files

| File | Purpose |
|------|---------|
| `Cargo.toml` | Manifest with `rigorix-engine` dependency |
| `src/main.rs` | Binary entry point |
| `src/lib.rs` | Library root |
| `src/cli_boundary/cli.rs` | Clap command definitions |
| `src/cli_boundary/dispatch.rs` | Command dispatch logic |
| `.pi/architecture/modules/cli-boundary.md` | Architecture doc |

## Commands

| Command | Purpose |
|---------|---------|
| `cargo build` | Build CLI |
| `cargo test` | Run tests |
| `cargo fmt --check` | Format check |
| `cargo clippy` | Lint |
| `cargo audit` | Security audit |
