//! Infrastructure layer for the Security Configuration bounded context.
//!
//! @canonical actions/.pi/architecture/modules/security-config.md#infrastructure
//! Implements: Contract Freeze — repository interfaces for fork detection, token validation,
//! policy loading, HMAC key management, and URL allowlisting
//! Issue: issue-contract-freeze
//!
//! This module defines the repository interfaces that abstract data access
//! for the Security Configuration module. Implementations will read from
//! environment variables, filesystem, GitHub API, and other sources.
//!
//! # Contract (Frozen)
//! - All repository methods are async
//! - All methods return `Result<_, SecurityError>`
//! - No dependencies on external frameworks

pub mod repository;

pub use repository::*;
