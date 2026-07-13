//! Infrastructure layer for the Execution Tools bounded context.
//!
//! @canonical .pi/architecture/modules/execution-tools.md#infrastructure
//! Implements: Contract Freeze — EngineFacadeConfig repository interfaces
//!
//! This module provides:
//! - Repository interface definitions (traits) for execution state
//! - Engine-facing client abstraction
//! - In-memory repository implementations
//!
//! # Contract (Frozen)
//!
//! - Repository traits only define contracts — no implementation in interfaces
//! - All methods are async
//! - All methods return domain error types

pub mod engine_facade_impl;
pub mod in_memory_repository;
pub mod repository;

pub use engine_facade_impl::{EngineFacadeConfig, EngineFacadeImpl};
pub use in_memory_repository::InMemoryExecutionRepository;
pub use repository::ExecutionRepository;
