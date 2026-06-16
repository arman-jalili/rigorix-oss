//! Observability domain types — error types, event schemas for CLI observability.
//!
//! @canonical .pi/architecture/modules/observability.md
//! Implements: Contract Freeze — ObservabilityCliError
//! Issue: issue-contract-freeze

pub mod error;
pub mod event;

pub use error::ObservabilityCliError;
