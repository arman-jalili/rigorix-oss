//! Cancellation domain types — error types, event schemas for CLI cancellation.
//!
//! @canonical .pi/architecture/modules/cancellation.md
//! Implements: Contract Freeze — CancellationCliError, CancellationCliEvent
//! Issue: issue-contract-freeze
//!
//! These types define the domain-level contracts for CLI cancellation operations.
//! They are pure domain objects with no framework dependencies.
//!
//! # Contract (Frozen)
//! - Errors carry structured context for diagnostics
//! - Events are serializable for logging and CI/CD output
//! - No implementation logic — only type definitions

pub mod error;
pub mod event;

pub use error::CancellationCliError;
pub use event::CancellationCliEvent;
