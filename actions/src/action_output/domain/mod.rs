//! Domain entities and interfaces for the Action Output bounded context.
//!
//! @canonical actions/.pi/architecture/modules/action-output.md#domain
//! Implements: Contract Freeze — domain entities FormattedOutput, WorkflowAnnotation,
//! StepSummary, OutputVariable, PrComment, ActionOutputError, ActionOutputEvent
//! Issue: issue-contract-freeze
//!
//! This module defines the core domain types — `FormattedOutput`, `WorkflowAnnotation`,
//! `StepSummary`, `OutputVariable`, `PrComment`, `ActionOutputError`, and
//! `ActionOutputEvent`. These are pure domain objects with no framework
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
pub mod types;

pub use error::*;
pub use event::*;
pub use types::*;
