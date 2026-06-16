//! Configuration domain types — CliConfig value object, error types, event schemas.
//!
//! @canonical .pi/architecture/modules/configuration.md
//! Implements: Contract Freeze — ConfigCliError, ConfigCliEvent
//! Issue: issue-contract-freeze
//!
//! These types define the domain-level contracts for CLI configuration operations.
//! They are pure domain objects with no framework dependencies.
//!
//! # Contract (Frozen)
//! - Errors carry structured context for diagnostics
//! - Events are serializable for logging and CI/CD output
//! - No implementation logic — only type definitions

pub mod config;
pub mod error;
pub mod event;

pub use config::*;
pub use error::ConfigCliError;
pub use event::ConfigCliEvent;
