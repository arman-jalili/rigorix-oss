//! Cancellation infrastructure — signal handler implementation, repository interfaces.
//!
//! @canonical .pi/architecture/modules/cancellation.md
//! Implements: Contract Freeze — repository interfaces, SignalHandlerImpl
//! Issue: issue-contract-freeze
//!
//! Infrastructure layer for CLI cancellation operations:
//! - `signal.rs` re-exports `SignalHandler` trait and `ShutdownLevel` from application/
//! - `signal_impl.rs` implements the trait via tokio signal handling
//! - `repository/` defines persistence interfaces for CLI-level signal state

pub mod repository;
pub mod signal;
pub mod signal_impl;
