//! Infrastructure layer for the Execution Tools bounded context.
//!
//! @canonical .pi/architecture/modules/execution-tools.md#infrastructure
//! Implements: Contract Freeze — EngineFacadeConfig repository interfaces
//!
//! This module provides:
//! - Repository interface definitions (traits) for execution state
//! - Engine-facing client abstraction
//!
//! # Contract (Frozen)
//!
//! - Repository traits only define contracts — no implementation in interfaces
//! - All methods are async
//! - All methods return domain error types

pub mod repository;
