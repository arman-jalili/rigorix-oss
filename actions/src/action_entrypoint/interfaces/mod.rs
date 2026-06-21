//! External interfaces for the Action Entrypoint bounded context.
//!
//! @canonical actions/.pi/architecture/modules/action-entrypoint.md#interfaces
//! Implements: Contract Freeze — HTTP API contracts
//! Issue: issue-contract-freeze
//!
//! These interfaces define how external systems interact with the action
//! entrypoint. Currently only HTTP endpoints are defined (for local
//! development, debugging, and testing).

pub mod http;

pub use http::*;
