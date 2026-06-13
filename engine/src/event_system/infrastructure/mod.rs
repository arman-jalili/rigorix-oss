//! Infrastructure layer interfaces for the Event System.
//!
//! @canonical .pi/architecture/modules/event-system.md
//! Implements: Contract Freeze — repository interfaces
//! Issue: #46
//!
//! This module defines repository interfaces that abstract data access
//! behind traits. Implementations are provided by the concrete
//! infrastructure module.

pub mod in_memory_event_repository;
pub mod repository;

pub use in_memory_event_repository::*;
pub use repository::*;
