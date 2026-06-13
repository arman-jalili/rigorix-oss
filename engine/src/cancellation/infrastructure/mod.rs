//! Infrastructure layer interfaces for the Cancellation bounded context.
//!
//! @canonical .pi/architecture/modules/cancellation.md
//! Implements: Contract Freeze — repository interfaces
//! Issue: issue-contract-freeze
//!
//! This module defines repository interfaces that abstract data access
//! behind traits. Implementations are provided by the concrete
//! infrastructure module.
//!
//! The Cancellation module does not have persistence requirements at this
//! time — cancellation state is purely in-memory. Repository interfaces
//! are reserved here for future use (e.g., persisting cancellation audit
//! records, storing cancellation policies).

pub mod repository;
