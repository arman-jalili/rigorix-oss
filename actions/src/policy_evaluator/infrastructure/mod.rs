//! Infrastructure layer for the Policy Evaluator bounded context.
//!
//! @canonical actions/.pi/architecture/modules/policy-evaluator.md#infrastructure
//! Implements: Contract Freeze — repository interfaces for policy data access
//! Issue: issue-contract-freeze
//!
//! This module defines the repository interfaces that abstract data access
//! for the Policy Evaluator module. Implementations will read from the
//! GitHub API (to fetch policy files from the base branch) and potentially
//! from filesystem or organization repositories.
//!
//! # Contract (Frozen)
//! - All repository methods are async
//! - All methods return `Result<_, PolicyError>`
//! - No dependencies on external frameworks

pub mod repository;

pub use repository::*;
