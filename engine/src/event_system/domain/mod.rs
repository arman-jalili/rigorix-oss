//! Domain entities and interfaces for the Event System bounded context.
//!
//! @canonical .pi/architecture/modules/event-system.md#domain
//! Implements: Contract Freeze — ExecutionEvent, PersistedEvent, EventSystemError
//! Issue: #46
//!
//! This module defines the core domain types:
//! - `ExecutionEvent` — Tagged union of all 11 execution event variants
//! - `PersistedEvent` — Event wrapper with monotonic sequence number
//! - `EventSystemError` — Domain error type for the event system
//!
//! These are pure domain objects with no framework dependencies.
//! They serve as the frozen contract that all implementation must satisfy.
//!
//! # Contract (Frozen)
//! - No implementation logic beyond constructors and field accessors
//! - All validation must happen in the application layer (service traits)
//! - All persistence must happen behind repository interfaces

pub mod error;
pub mod event;

pub use error::EventSystemError;
pub use event::*;
