//! Domain entities and interfaces for the Policy Evaluator bounded context.
//!
//! @canonical actions/.pi/architecture/modules/policy-evaluator.md#domain
//! Implements: Contract Freeze — domain entities PolicyDocument, PolicyRules,
//! DenyRule, ReviewRule, FlagRule, Severity, PolicyLimits, AuditConfig,
//! PolicyViolation, PolicyResult, ViolationCounts, CompiledRules,
//! PolicyError, PolicyEvent
//! Issue: issue-contract-freeze
//!
//! This module defines the core domain types for policy evaluation.
//! These are pure domain objects with no framework dependencies.
//! They serve as the frozen contract that all implementation must satisfy.
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
