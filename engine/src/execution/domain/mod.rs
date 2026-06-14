//! Domain entities and interfaces for the Execution bounded context.
//!
//! @canonical .pi/architecture/modules/error-handling.md#execution
//! Implements: Contract Freeze — ExecutionError
//! Issue: #186
//!
//! # Contract (Frozen)
//! - `ExecutionError` is the single error type for execution operations
//! - All execution domain types are pure — no framework dependencies

pub mod error;

pub use error::*;
