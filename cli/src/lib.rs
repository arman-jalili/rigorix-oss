//! CLI library root.
//!
//! @canonical .pi/architecture/modules/cli-boundary.md
//! Implements: Contract Freeze — CLI boundary module root
//! Issue: issue-contract-freeze
//!
//! The CLI is a thin wrapper around `rigorix-engine`. It handles:
//! - Command parsing (clap)
//! - Config loading and merging
//! - TUI rendering
//! - Output formatting
//! - Signal handling (Ctrl+C)
//!
//! Per ADR-002, no business logic lives here — all execution, planning,
//! and domain logic lives in the engine crate.
//!
//! # Architecture
//!
//! ```text
//! cli/src/
//! ├── domain/           # CLI-specific domain types, errors, events
//! │   ├── error.rs      # CliError enum
//! │   ├── config.rs     # CliConfig value object
//! │   └── event/        # CLI event payload schemas
//! ├── application/      # Service traits, DTOs, factory interfaces
//! │   ├── service.rs    # CliOrchestrator, ExecutionSession traits
//! │   ├── factory.rs    # Factory interfaces
//! │   └── dto/          # Input/Output DTOs with validation
//! ├── infrastructure/   # I/O concern interfaces
//! │   ├── config.rs     # CliConfigLoader trait
//! │   ├── output.rs     # LogFormatter trait
//! │   ├── signal.rs     # SignalHandler trait
//! │   └── repository/   # (reserved for future CLI state persistence)
//! ├── interfaces/       # API contracts
//! │   └── cli/          # Clap CLI command definitions
//! └── tui/              # Ratatui TUI renderer
//! ```
//!
//! # Contract Freeze Notice
//!
//! ALL files in this module are frozen contracts.
//! - No implementation changes without explicit contract change approval
//! - Implementation PRs MUST reference these interfaces
//! - DTO schemas serve as the canonical data contract

pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod interfaces;
pub mod tui;

#[cfg(test)]
pub mod tests;
