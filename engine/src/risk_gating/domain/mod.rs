//! Domain entities and interfaces for the Risk Gating bounded context.
//!
//! @canonical .pi/architecture/modules/risk-gating.md#domain
//! Implements: Contract Freeze — domain entities RiskLevel, RiskClassifier trait, RiskConfig, RiskGatingError, RiskGateEvent
//! Issue: issue-contract-freeze
//!
//! This module defines the core domain types — `RiskLevel`, `RiskClassifier` trait,
//! `RiskConfig`, `RiskGatingError`, and all risk-gate-related events. These are
//! pure domain objects with no framework dependencies. They serve as the frozen
//! contract that all implementation must satisfy.
//!
//! # Contract Freeze
//! - No implementation logic beyond constructors and field accessors
//! - All validation must happen in the application layer (service traits)
//! - All persistence must happen behind repository interfaces

pub mod error;
pub mod event;
pub mod risk_classifier;
pub mod risk_config;
pub mod risk_level;

pub use error::RiskGatingError;
pub use risk_classifier::RiskClassifier;
pub use risk_config::RiskConfig;
pub use risk_level::RiskLevel;
