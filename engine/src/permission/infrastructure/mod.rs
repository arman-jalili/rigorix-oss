//! Infrastructure layer interfaces for the Permission Enforcer bounded context.
//!
//! @canonical .pi/architecture/modules/permission-enforcer.md
//! Implements: Contract Freeze — repository interfaces
//! Issue: issue-contract-freeze
//!
//! This module defines repository interfaces that abstract data access
//! behind traits. Implementations are provided by the concrete
//! infrastructure module.
//!
//! The primary repository is `PermissionConfigRepository` for loading
//! and persisting permission configuration.

pub mod repository;

pub use repository::*;
