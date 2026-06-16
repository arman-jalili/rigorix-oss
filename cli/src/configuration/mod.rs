//! Configuration module — CLI-side config loading and merging.
//!
//! @canonical .pi/architecture/modules/configuration.md
//! Implements: Contract Freeze — CLI Configuration module (interfaces only)
//! Issue: issue-contract-freeze
//!
//! Multi-source configuration loading with layered merging.
//! CLI flags → env vars → rigorix.toml → engine defaults.
//!
//! # Architecture (Clean Architecture layers)
//!
//! ```text
//! configuration/
//! ├── domain/           # CliConfig, ConfigCliError, ConfigCliEvent
//! │   ├── mod.rs
//! │   ├── config.rs     # CliConfig value object
//! │   ├── error.rs      # ConfigCliError enum
//! │   └── event/        # ConfigCliEvent payload schemas
//! │       └── mod.rs
//! ├── application/      # Service traits, DTO schemas
//! │   ├── mod.rs
//! │   ├── service.rs    # CliConfigLoader trait
//! │   └── dto/          # LoadConfigInput/Output, ValidateConfig types
//! │       └── mod.rs
//! ├── infrastructure/   # Trait implementations, repository interfaces
//! │   ├── mod.rs
//! │   ├── config.rs                   # Re-exports CliConfigLoader
//! │   ├── config_impl.rs              # CliConfigLoaderImpl
//! │   └── repository/                # ConfigCliRepository trait
//! │       └── mod.rs
//! └── interfaces/       # HTTP API contracts
//!     ├── mod.rs
//!     └── http/         # Endpoint definitions, request/response schemas
//!         └── mod.rs
//! ```
//!
//! # Contract Freeze Notice
//!
//! ALL interface files in this module are frozen contracts.
//! - No implementation changes without explicit contract change approval
//! - Implementation PRs MUST reference these interfaces
//! - DTO schemas serve as the canonical data contract
//! - The CliConfigLoader trait is the primary service contract

pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod interfaces;
