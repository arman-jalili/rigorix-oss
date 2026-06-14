//! Domain entities and interfaces for the DAG Engine bounded context.
//!
//! @canonical .pi/architecture/modules/dag-engine.md#domain
//! Implements: Contract Freeze — domain entities TaskGraph, TaskNode,
//! ExecutionPolicy, ValidationRule, PlanDiff, ImpactLevel
//! Issue: issue-contract-freeze
//!
//! This module defines the core domain types for DAG construction and planning.
//! These are pure domain objects with no framework dependencies. They serve as
//! the frozen contract that all implementation must satisfy.
//!
//! # Contract (Frozen)
//! - No implementation logic beyond constructors and field accessors
//! - All validation must happen in the application layer (service traits)
//! - All persistence must happen behind repository interfaces
//! - All domain types are serializable (Serialize + Deserialize)

pub mod error;
pub mod event;
pub mod graph;
pub mod plan;

pub use error::*;
pub use graph::*;
pub use plan::*;
