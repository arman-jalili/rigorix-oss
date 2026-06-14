//! Infrastructure layer interfaces for the State Persistence bounded context.
//!
//! @canonical .pi/architecture/modules/state-persistence.md
//! Implements: Contract Freeze — repository interfaces
//! Issue: issue-contract-freeze
//!
//! This module defines repository interfaces that abstract data access
//! behind traits. Implementations are provided by the concrete
//! infrastructure module.
//!
//! The primary repositories are:
//! - `StateRepository` — for CRUD on execution state files
//! - `GraphRepository` — for CRUD on execution graph records

pub mod repository;

pub use repository::*;
