//! Domain layer for the Audit Tools bounded context.
//!
//! @canonical .pi/architecture/modules/audit-tools.md#domain
//! Implements: Contract Freeze — AuditQueryService trait, value objects, events, error types
//!
//! This module defines the core domain types for Audit Tools:
//! - Aggregate root: `AuditQueryService` trait
//! - Value objects: `AuditFilter`, `AuditSummary`, `TopFailure`, `TopTemplate`
//! - Domain events: `AuditToolsEvent`
//! - Error types: `AuditError`, `HandlerError`
//! - Formatter: `AuditFormatter` trait
//!
//! These are pure domain types with no framework dependencies. They serve as
//! the frozen contract that all implementation must satisfy.
//!
//! # Contract (Frozen)
//!
//! - No implementation logic beyond constructors and field accessors
//! - All types are serializable (Serialize + Deserialize)
//! - Value objects are immutable (no pub fields, no setters)
//! - AuditQueryService exposes trait methods, not pub fields
//! - Domain events carry execution_id/session_id and timestamp

pub mod entity;
pub mod error;
pub mod event;
pub mod formatter_impl;
pub mod value;

pub use entity::{AuditFormatter, AuditQueryService, SharedAuditQueryService};
pub use error::{AuditError, AuditHandlerError};
pub use event::AuditToolsEvent;
pub use formatter_impl::AuditFormatterImpl;
pub use value::{
    AuditEnvelope, AuditFilter, AuditSummary, EventStatus, ExecutionEvent, TopFailure, TopTemplate,
};
