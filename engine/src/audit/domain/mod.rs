//! Domain entities and interfaces for the Audit bounded context.
//!
//! @canonical .pi/architecture/modules/audit.md#domain
//! Implements: Contract Freeze — domain entities AuditEnvelope, AuditError, AuditEvent
//! Issue: #13
//!
//! This module defines the core domain types — `AuditEnvelope`, `AuditError`,
//! and `CircuitBreakerState`. These are pure domain objects with no framework
//! dependencies. They serve as the frozen contract that all implementation
//! must satisfy.
//!
//! # Contract Freeze
//! - No implementation logic beyond constructors and field accessors
//! - All validation must happen in the application layer (service traits)
//! - All persistence must happen behind repository interfaces

pub mod envelope;
pub mod error;
pub mod event;

pub use envelope::*;
pub use error::AuditError;
