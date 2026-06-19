//! Domain entities and interfaces for the Quality Gates bounded context.
//!
//! @canonical .pi/architecture/modules/quality-gates.md
//! Implements: Contract Freeze — QualityLevel, GreenContract, QualityGateOutcome,
//!              QualityGateConfig, QualityGateEvent, QualityGateError
//! Issue: #449 (quality-gates epic)
//!
//! This module defines the core domain types — `QualityLevel`, `GreenContract`,
//! `QualityGateOutcome`, `QualityGateConfig`, `QualityGateEvent`, and
//! `QualityGateError`. These are pure domain objects with no framework
//! dependencies. They serve as the frozen contract that all implementations
//! must satisfy.
//!
//! # Contract Freeze
//! - No implementation logic beyond enum variants, accessors, and constructors
//! - All quality gate orchestration logic must happen in the application layer
//! - All persistence must happen behind repository interfaces
//! - The QualityLevel ↔ GreenContract evaluation is the core domain invariant

pub mod config;
pub mod contract;
pub mod error;
pub mod event;
pub mod level;
pub mod outcome;

pub use config::QualityGateConfig;
pub use contract::GreenContract;
pub use error::QualityGateError;
pub use event::QualityGateEvent;
pub use level::QualityLevel;
pub use outcome::QualityGateOutcome;
