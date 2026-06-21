//! Infrastructure layer for the CI Integration bounded context.
//!
//! @canonical actions/.pi/architecture/modules/ci-integration.md#infrastructure
//! Implements: Contract Freeze — repository interfaces for status checks,
//! PR comments, and execution tracking data access
//! Issue: issue-contract-freeze
//!
//! This module defines the repository interfaces that abstract data access
//! for the CI Integration module. Implementations will communicate with the
//! GitHub REST API via the shared `GitHubClient`.
//!
//! # Contract (Frozen)
//! - All repository methods are async
//! - All methods return `Result<_, CiIntegrationError>`
//! - No dependencies on external frameworks

pub mod repository;

pub use repository::*;
