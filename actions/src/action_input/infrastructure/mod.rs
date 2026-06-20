//! Infrastructure layer for the Action Input bounded context.
//!
//! @canonical actions/.pi/architecture/modules/action-input.md#infrastructure
//! Implements: Contract Freeze — repository interfaces for input, config, and event data access
//! Issue: issue-contract-freeze
//!
//! This module defines the repository interfaces that abstract data access
//! for the Action Input module. Implementations will read from environment
//! variables, filesystem (action.yml, event payloads), and CLI arguments.
//!
//! # Contract (Frozen)
//! - All repository methods are async
//! - All methods return `Result<_, ActionInputError>`
//! - No dependencies on external frameworks

pub mod repository;

pub use repository::*;
