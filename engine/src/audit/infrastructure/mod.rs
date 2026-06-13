//! Infrastructure layer interfaces for the Audit bounded context.
//!
//! @canonical .pi/architecture/modules/audit.md
//! Implements: Contract Freeze — repository interfaces
//! Issue: #13
//!
//! This module defines repository interfaces that abstract data access
//! behind traits. Implementations are provided by the concrete
//! infrastructure module.

pub mod local_audit_repository;
pub mod repository;

pub use local_audit_repository::*;
