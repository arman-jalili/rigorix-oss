//! Infrastructure layer for the LLM Step bounded context.
//!
//! @canonical .pi/architecture/modules/llm-step.md#infrastructure
//! Implements: Contract Freeze — repository interfaces
//! Issue: issue-contract-freeze
//!
//! This module defines the repository interfaces for LLM step persistence.
//! Implementations may use filesystem, database, or in-memory storage.
//!
//! # Contract (Frozen)
//! - All repository methods are async
//! - All methods return domain error types
//! - No framework-specific annotations on trait definitions

pub mod repository;
