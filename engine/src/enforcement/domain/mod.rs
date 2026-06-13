//! Domain entities and interfaces for the Enforcement bounded context.
//!
//! @canonical .pi/architecture/modules/enforcement.md#domain
//! Implements: Contract Freeze — domain entities EnforcementConfig, EnforcementError, EnforcementEvent
//! Issue: issue-contract-freeze
//!
//! This module defines the core domain types — `EnforcementConfig`, `EnforcementError`,
//! and all enforcement-related events. These are pure domain objects with no
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

pub use config::{ConfigValidationError, EnforcementConfig, EnforcementPresetProfile, ExecutionLimits, ResourceBudget, SafetyCaps, ToolPolicy, ToolRiskLevel};
pub use error::EnforcementError;
