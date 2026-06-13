//! Configuration bounded context.
//!
//! This module handles loading and validating configuration from
//! `rigorix.toml`, environment variables, and programmatic defaults
//! with layered merging. It also provides the `Secret` type for
//! safe API key handling.
//!
//! # Architecture
//!
//! ```text
//! configuration/
//! ├── domain/           # Domain entities (Config, Secret), errors, events
//! │   ├── config.rs     # Config aggregate and all sub-configs
//! │   ├── secret.rs     # Secret value object (redacted debug/display)
//! │   ├── error.rs      # ConfigurationError enum
//! │   └── event/        # ConfigurationEvent payload schemas
//! ├── application/      # Service traits, DTOs, factory interfaces
//! │   ├── service.rs    # ConfigService, SecretService traits
//! │   ├── factory.rs    # ConfigFactory, SecretFactory traits
//! │   └── dto/          # Input/Output DTOs with validation
//! ├── infrastructure/   # Repository interfaces
//! │   └── repository/   # ConfigRepository, ConfigWriteRepository
//! └── interfaces/       # API contracts
//!     └── http/         # REST endpoint contracts
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
