//! Domain entities and interfaces for the State Persistence bounded context.
//!
//! @canonical .pi/architecture/modules/state-persistence.md#domain
//! Implements: Contract Freeze — domain entities ExecutionState, NodeState, StateError, StateEvent
//! Issue: issue-contract-freeze
//!
//! This module defines the core domain types — `ExecutionState`, `NodeState`,
//! `ExecutionStatus`, `NodeStatus`, `StateError`, `ExecutionGraph`, and all
//! state-related events. These are pure domain objects with no framework
//! dependencies. They serve as the frozen contract that all implementation
//! must satisfy.
//!
//! # Contract (Frozen)
//! - No implementation logic beyond constructors and field accessors
//! - All validation must happen in the application layer (service traits)
//! - All persistence must happen behind repository interfaces

pub mod context;
pub mod error;
pub mod event;
pub mod graph;
pub mod state;

pub use context::*;
pub use error::StateError;
pub use graph::*;
pub use state::*;
