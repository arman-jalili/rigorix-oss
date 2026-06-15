//! Infrastructure layer interfaces for the Planning Pipeline bounded context.
//!
//! @canonical .pi/architecture/modules/planning-pipeline.md#infrastructure
//! Implements: Contract Freeze — repository interfaces
//! Issue: issue-contract-freeze
//!
//! This module defines repository interfaces that abstract data access
//! behind traits. Implementations are provided by the concrete
//! infrastructure module.
//!
//! The primary repository is `PlanningResultRepository` for persisting
//! and loading planning results and their deterministic hashes.

pub mod claude_classifier;
pub mod openai_classifier;
pub mod repository;

pub use claude_classifier::*;
pub use openai_classifier::*;
pub use repository::*;
