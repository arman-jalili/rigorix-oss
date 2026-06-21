//! Infrastructure layer for the Action Entrypoint bounded context.
//!
//! @canonical actions/.pi/architecture/modules/action-entrypoint.md#infrastructure
//! Implements: Contract Freeze — repository interfaces for context data access
//! Issue: issue-contract-freeze
//!
//! This module defines the repository interfaces that abstract data access
//! for the Action Entrypoint module. Implementations will read from environment
//! variables, filesystem (event payloads), and CLI arguments.
//!
//! # Contract (Frozen)
//! - All repository methods are async
//! - All methods return `Result<_, ActionError>`
//! - No dependencies on external frameworks

pub mod context_repository_impl;
pub mod repository;

pub use context_repository_impl::*;
pub use repository::*;
