//! Infrastructure layer for the Execution Engine bounded context.
//!
//! @canonical .pi/architecture/modules/execution-engine.md#infrastructure
//! Implements: Contract Freeze — repository interfaces for execution state persistence
//! Issue: issue-contract-freeze
//!
//! This module contains repository interfaces for persisting execution state
//! (for crash recovery, audit, and replay) and execution results.
//!
//! # Contract (Frozen)
//! - All repository methods are async
//! - All methods return domain error types
//! - No framework-specific annotations on trait definitions

pub mod repository;
