//! Domain entities and interfaces for the Cancellation bounded context.
//!
//! @canonical .pi/architecture/modules/cancellation.md#domain
//! Implements: Contract Freeze — domain entities ShutdownSignal, CancellationError, CancellationEvent
//! Issue: issue-contract-freeze
//!
//! This module defines the core domain types — `ShutdownSignal`, `CancellationError`,
//! and all cancellation-related events. These are pure domain objects with no
//! framework dependencies. They serve as the frozen contract that all implementation
//! must satisfy.
//!
//! # Contract Freeze
//! - No implementation logic beyond constructors and field accessors
//! - All validation must happen in the application layer (service traits)
//! - All persistence must happen behind repository interfaces

pub mod error;
pub mod event;
pub mod signal;

pub use error::CancellationError;
pub use signal::ShutdownSignal;
