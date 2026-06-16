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
//! Each bounded context is a self-contained module with Clean Architecture
//! layers (domain → application → infrastructure → interfaces):
//!
//! ```text
//! cli/src/
//! ├── cli-boundary/     # Command dispatch, output, TUI, shared CLI types
//! │   ├── domain/       # CliError, CliEvent enums
//! │   ├── application/  # CliOrchestrator, ExecutionSession traits, DTOs
//! │   ├── infrastructure/ # LogFormatter trait + impl, repository
//! │   ├── interfaces/   # Clap CLI command definitions
//! │   └── tui/          # Ratatui TUI renderer
//! ├── configuration/    # Multi-source config loading
//! │   ├── domain/       # CliConfig, ConfigCliError, ConfigCliEvent
//! │   ├── application/  # CliConfigLoader trait, DTO schemas
//! │   ├── infrastructure/ # CliConfigLoaderImpl, ConfigCliRepository
//! │   └── interfaces/   # HTTP API contracts
//! ├── observability/    # Tracing, health checks, event schemas
//! │   ├── domain/       # ObservabilityEvent schemas
//! │   └── infrastructure/ # TracingInitializer trait + tracing impl
//! ├── cancellation/     # Signal handler for Ctrl+C
//! │   ├── domain/       # CancellationCliError, CancellationCliEvent
//! │   ├── application/  # SignalHandler trait, DTO schemas
//! │   ├── infrastructure/ # SignalHandlerImpl, CancellationCliRepository
//! │   └── interfaces/   # HTTP API contracts
//! └── templates/        # Template list/show commands
//!     ├── domain/       # TemplateCliError, TemplateCliEvent
//!     ├── application/  # TemplateCommandService trait, DTOs
//!     ├── infrastructure/ # TemplateEngineHandler, TemplateCliRepository
//!     └── interfaces/   # HTTP API contracts
//! ```
//!
//! # Contract Freeze Notice
//!
//! ALL files in this module are frozen contracts.
//! - No implementation changes without explicit contract change approval
//! - Implementation PRs MUST reference these interfaces
//! - DTO schemas serve as the canonical data contract

pub mod cancellation;
pub mod cli_boundary;
pub mod configuration;
pub mod observability;
pub mod templates;

// Tests are defined in cli_boundary::tests which is #[cfg(test)] only.
// They are discovered by cargo because the module is declared from
// cli_boundary/mod.rs, not from here.
