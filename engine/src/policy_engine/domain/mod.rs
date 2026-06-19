//! Domain entities and interfaces for the Policy Engine bounded context.
//!
//! @canonical .pi/architecture/modules/policy-engine.md#domain
//! Implements: Contract Freeze — domain entities PolicyRule, PolicyCondition,
//!   PolicyAction, LaneContext, PolicyConfig, PolicyEngineError, PolicyEvent
//! Issue: issue-contract-freeze
//!
//! This module defines the core domain types — the composable policy condition
//! tree, the priority-ordered rule engine, execution actions, and the typed
//! execution context (LaneContext) evaluated by conditions. These are pure
//! domain objects with no framework dependencies.
//!
//! # Contract (Frozen)
//! - No implementation logic beyond constructors and field accessors
//! - All evaluation orchestration lives in the application layer
//! - All persistence happens behind repository interfaces

pub mod action;
pub mod condition;
pub mod config;
pub mod context;
pub mod error;
pub mod event;
pub mod rule;

pub use action::{PolicyAction, ReconcileReason};
pub use condition::PolicyCondition;
pub use config::{PolicyConfig, RuleDefinition};
pub use context::{DiffScope, LaneBlocker, LaneContext, ReviewStatus};
pub use error::PolicyEngineError;
pub use event::PolicyEvent;
pub use rule::PolicyRule;
