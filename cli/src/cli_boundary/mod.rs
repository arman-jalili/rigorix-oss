//! CLI boundary module — thin wrapper around `rigorix-engine`.
//!
//! @canonical .pi/architecture/modules/cli-boundary.md
//! Implements: Contract Freeze — module root
//! Issue: issue-contract-freeze
//!
//! This module provides the flag-based CLI interface for scripting and CI/CD.
//! For the interactive TUI, see the `tui` module.
//!
//! # Contract (Frozen)
//!
//! The public API of this module is:
//! - `cli::parse_args()` → `CliCommand`
//! - `dispatch::dispatch(command, config, cancellation_token)` → `DispatchResult`
//! - `orchestrator::build_orchestrator(config, cancellation_token)` → `Box<dyn OrchestratorService>`
//! - `config::load_config()` → `Config`
//! - `output::format_and_exit(result)` → never returns (process exit)
//! - `signal::install_signal_handler()` → `CancellationToken`
//! - `tracing::init_tracing()` → nothing
//! - `error::CliError` → exit code mapping

pub mod cli;
pub mod config;
pub mod dispatch;
pub mod error;
pub mod orchestrator;
pub mod output;
pub mod signal;
pub mod tracing;

#[cfg(test)]
pub mod tests;
