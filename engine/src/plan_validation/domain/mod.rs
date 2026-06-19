//! Domain entities and interfaces for the Plan Validation bounded context.
//!
//! @canonical .pi/architecture/modules/plan-validation.md#domain
//! Implements: Contract Freeze — ValidationLoopConfig, ValidationState, ValidationOutcome,
//! ValidationReport, ValidationLoopError, ValidationEvent
//! Issue: issue-contract-freeze
//!
//! This module defines the core domain types — `ValidationLoopConfig`, `ValidationState`,
//! `ValidationOutcome`, `ValidationReport`, `ValidationIterationReport`, `ValidationLoopError`,
//! and `ValidationEvent`. These are pure domain objects with no framework dependencies.
//! They serve as the frozen contract that all implementation must satisfy.
//!
//! # Contract (Frozen)
//! - No implementation logic beyond constructors and field accessors
//! - All validation must happen in the application layer (service traits)
//! - All persistence must happen behind repository interfaces
//! - All domain types are serializable (Serialize + Deserialize)

pub mod error;
pub mod event;
pub mod loop_config;
pub mod outcome;
pub mod report;
pub mod state;

pub use error::*;
pub use event::*;
pub use loop_config::*;
pub use outcome::*;
pub use report::*;
pub use state::*;
