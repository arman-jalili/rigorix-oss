//! Domain entities and interfaces for the Orchestrator bounded context.
//!
//! @canonical .pi/architecture/modules/orchestrator.md#domain
//! Implements: Contract Freeze — domain entities ExecutionRecord, OrchestratorConfig,
//!                                     OrchestratorError, OrchestratorEvent
//! Issue: #338
//!
//! This module defines the core domain types — `ExecutionRecord`, `OrchestratorConfig`,
//! `OrchestratorError`, and `OrchestratorEvent`. These are pure domain objects with no
//! framework dependencies. They serve as the frozen contract that all implementation
//! must satisfy.
//!
//! # Contract Freeze
//! - No implementation logic beyond constructors and field accessors
//! - All validation must happen in the application layer (service traits)
//! - All persistence must happen behind repository interfaces

pub mod config;
pub mod error;
pub mod event;
pub mod record;

pub use config::OrchestratorConfig;
pub use error::OrchestratorError;
pub use record::ExecutionRecord;
