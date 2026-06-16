//! Rigorix CLI — thin binary wrapper around `rigorix-engine`.
//!
//! @canonical .pi/architecture/modules/cli-boundary.md
//! Implements: Contract Freeze — CLI library root
//! Issue: issue-contract-freeze
//!
//! # Architecture
//!
//! Per ADR-002, the CLI contains zero business logic — all execution, planning,
//! and domain logic lives in the engine crate. The CLI is responsible only for:
//!
//! - Command parsing (Clap)
//! - Config loading and merging (TOML + env + flags)
//! - Output formatting (Pretty, JSON, Markdown, Quiet)
//! - Signal handling (Ctrl+C, SIGTERM)
//! - Tracing initialization
//! - Dispatching to engine services
//!
//! # Module Structure
//!
//! ```text
//! src/
//! ├── main.rs            # Binary entry point
//! ├── lib.rs             # Library root ← this file
//! ├── cli_boundary/      # Flag-based CLI (scripts, CI/CD)
//! │   ├── mod.rs
//! │   ├── cli.rs         # Clap: 14 commands + flags + shortcuts
//! │   ├── dispatch.rs    # Command → engine service routing
//! │   ├── orchestrator.rs# OrchestratorBuilder wrapper
//! │   ├── config.rs      # Multi-source config loader
//! │   ├── output.rs      # LogFormatter trait + types
//! │   ├── output_impl.rs # Formatter implementations
//! │   ├── signal.rs      # Ctrl+C/SIGTERM handler
//! │   ├── tracing.rs     # tracing-subscriber init
//! │   ├── error.rs       # CliError → exit codes
//! │   └── tests.rs       # Integration tests
//! └── tui/               # Terminal UI (interactive)
//!     └── ...            # Defined in tui.md
//! ```
//!
//! # Contract (Frozen)
//!
//! These public module declarations and the associated sub-module traits/types
//! constitute the frozen contract for the cli-boundary epic. Implementation
//! issues must satisfy these interfaces without modifying them.
//!
//! ## Components
//!
//! | Component | Module | Status |
//! |-----------|--------|--------|
//! | CliParser | `cli_boundary::cli` | Planned |
//! | Dispatcher | `cli_boundary::dispatch` | Planned |
//! | OrchestratorBuilder | `cli_boundary::orchestrator` | Planned |
//! | ConfigLoader | `cli_boundary::config` | Planned |
//! | OutputFormatter | `cli_boundary::output` | Planned |
//! | SignalHandler | `cli_boundary::signal` | Planned |
//! | TracingInit | `cli_boundary::tracing` | Planned |
//! | CliError | `cli_boundary::error` | Planned |
//! | TuiRoot | `tui` | Planned |

pub mod cli_boundary;
pub mod tui;
