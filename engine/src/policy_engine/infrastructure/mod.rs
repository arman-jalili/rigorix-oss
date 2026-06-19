//! Infrastructure layer interfaces for the Policy Engine bounded context.
//!
//! @canonical .pi/architecture/modules/policy-engine.md
//! Implements: Contract Freeze — repository interfaces
//! Issue: issue-contract-freeze
//!
//! This module defines repository interfaces that abstract data access
//! behind traits. Implementations are provided by the concrete
//! infrastructure module.
//!
//! The primary repository is `PolicyRepository` for loading and persisting
//! policy rules from various sources (file, database, remote API).

pub mod repository;

pub use repository::*;
