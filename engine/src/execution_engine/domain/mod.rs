//! Domain entities and interfaces for the Execution Engine bounded context.
//!
//! @canonical .pi/architecture/modules/execution-engine.md#domain
//! Implements: Contract Freeze — domain entities ParallelExecutor, RetryPolicy,
//! ExecutionResult, NodeExecutionState, FailureContext
//! Issue: issue-contract-freeze
//!
//! This module defines the core domain types for parallel DAG execution and
//! retry logic. These are pure domain objects with no framework dependencies.
//! They serve as the frozen contract that all implementation must satisfy.
//!
//! # Contract (Frozen)
//! - No implementation logic beyond constructors and field accessors
//! - All orchestration must happen in the application layer (service traits)
//! - All persistence must happen behind repository interfaces
//! - All domain types are serializable (Serialize + Deserialize)

pub mod error;
pub mod event;
pub mod parallel_executor;
pub mod retry;

pub use error::*;
pub use parallel_executor::*;
pub use retry::*;
