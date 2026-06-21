//! Domain entities and interfaces for the Audit Posting bounded context.
//!
//! @canonical actions/.pi/architecture/modules/audit-posting.md#domain
//! Implements: Contract Freeze — domain entities SignedAuditRecord, AuditPostingError,
//! AuditPostingEvent
//! Issue: issue-contract-freeze
//!
//! This module defines the core domain types — `SignedAuditRecord`, `AuditPostingError`,
//! and `AuditPostingEvent`. These are pure domain objects with no framework
//! dependencies. They serve as the frozen contract that all implementation
//! must satisfy.
//!
//! # Contract (Frozen)
//! - No implementation logic beyond constructors and field accessors
//! - All validation must happen in the application layer (service traits)
//! - All persistence must happen behind repository interfaces
//! - All domain types are serializable (Serialize + Deserialize)

pub mod error;
pub mod event;
pub mod signed_audit_record;

pub use error::*;
pub use event::*;
pub use signed_audit_record::*;
