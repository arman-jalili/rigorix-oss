//! Domain entities and interfaces for the Failure Classification bounded context.
//!
//! @canonical .pi/architecture/modules/failure-classification.md#types
//! Implements: Contract Freeze — domain entities FailureType, RetryStrategy,
//!              FailureClassificationError, FailureClassificationEvent
//! Issue: #33
//!
//! This module defines the core domain types — `FailureType`, `RetryStrategy`,
//! and the failure-to-strategy mapping. These are pure domain objects with no
//! framework dependencies. They serve as the frozen contract that all
//! implementation must satisfy.
//!
//! # Contract Freeze
//! - No implementation logic beyond enum variants and accessors
//! - All classification logic must happen in the application layer (service traits)
//! - All persistence must happen behind repository interfaces
//! - The FailureType ↔ RetryStrategy mapping is the core domain invariant

pub mod error;
pub mod event;
pub mod failure_type;
pub mod retry_strategy;

pub use error::FailureClassificationError;
pub use event::*;
pub use failure_type::FailureType;
pub use retry_strategy::RetryStrategy;
