//! Infrastructure layer interfaces for the Enforcement bounded context.
//!
//! @canonical .pi/architecture/modules/enforcement.md
//! Implements: Contract Freeze — repository interfaces
//! Issue: issue-contract-freeze
//!
//! This module defines repository interfaces that abstract data access
//! behind traits. Implementations are provided by the concrete
//! infrastructure module.
//!
//! The primary repository is `EnforcementPolicyRepository` for loading
//! and persisting enforcement policies and budgets. The default
//! implementation loads from the global `Config` object.

pub mod default_policy_repository;
pub mod repository;

pub use default_policy_repository::*;
pub use repository::*;
