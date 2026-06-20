//! Infrastructure layer for the Action Output bounded context.
//!
//! @canonical actions/.pi/architecture/modules/action-output.md
//! Implements: Contract Freeze — repository interfaces for output, summary, and GitHub API
//! Issue: issue-contract-freeze
//!
//! This module defines the infrastructure-level contracts:
//! - Repository interfaces in `repository/`
//!
//! # Contract (Frozen)
//! - All repository methods are async
//! - All methods return domain error types
//! - No framework-specific annotations on trait definitions
//! - Implementations are hidden behind these interfaces

pub mod repository;

pub use repository::*;
