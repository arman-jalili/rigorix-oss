//! Domain entities and interfaces for the Permission Enforcer bounded context.
//!
//! @canonical .pi/architecture/modules/permission-enforcer.md#domain
//! Implements: Contract Freeze — domain entities PermissionMode, PermissionPolicy,
//!   PermissionOutcome, BashClassifier, PermissionConfig, PermissionError, PermissionEvent
//! Issue: issue-contract-freeze
//!
//! This module defines the core domain types — the three-tier permission
//! mode, the authorization policy, bash command classification, and all
//! permission-related events. These are pure domain objects with no
//! framework dependencies.
//!
//! # Contract (Frozen)
//! - No implementation logic beyond constructors and field accessors
//! - All enforcement orchestration lives in the application layer
//! - All persistence happens behind repository interfaces

pub mod bash_classifier;
pub mod config;
pub mod context;
pub mod error;
pub mod event;
pub mod mode;
pub mod outcome;
pub mod policy;
pub mod prompter;

pub use bash_classifier::{BashClassifier, CommandIntent};
pub use config::PermissionConfig;
pub use context::PermissionContext;
pub use error::PermissionError;
pub use mode::PermissionMode;
pub use outcome::{EnforcementResult, PermissionOutcome};
pub use policy::PermissionPolicy;
pub use prompter::{AllowAllPrompter, DenyAllPrompter, PermissionPrompter, PromptResponse};
