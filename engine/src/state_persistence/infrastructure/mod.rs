//! Infrastructure layer interfaces and implementations for the State
//! Persistence bounded context.
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
//!
//! Implementations:
//! - `FileSystemStateRepository` — filesystem-backed state storage
//!   with atomic write-rename crash safety

pub mod filesystem_execution_record_repository;
pub mod filesystem_graph_repository;
pub mod filesystem_state_repository;
pub mod repository;

pub use filesystem_execution_record_repository::*;
pub use filesystem_graph_repository::*;
pub use filesystem_state_repository::*;
pub use repository::*;
