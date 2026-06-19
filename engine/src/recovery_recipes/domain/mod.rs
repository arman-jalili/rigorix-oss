//! Domain entities and interfaces for the Recovery Recipes bounded context.
//!
//! @canonical .pi/architecture/modules/recovery-recipes.md
//! Implements: Contract Freeze — FailureScenario, RecoveryStep, RecoveryRecipe,
//!              EscalationPolicy, RecoveryResult, RecoveryEvent, RecoveryError
//! Issue: #438 (recovery-recipes epic)
//!
//! This module defines the core domain types — `FailureScenario`,
//! `RecoveryStep`, `RecoveryRecipe`, `EscalationPolicy`, `RecoveryResult`,
//! `RecoveryEvent`, and `RecoveryError`. These are pure domain objects with
//! no framework dependencies. They serve as the frozen contract that all
//! implementations must satisfy.
//!
//! # Contract Freeze
//! - No implementation logic beyond enum variants, accessors, and constructors
//! - All recovery orchestration logic must happen in the application layer
//! - All persistence must happen behind repository interfaces
//! - The FailureScenario ↔ RecoveryRecipe mapping is the core domain invariant

pub mod error;
pub mod escalation;
pub mod event;
pub mod recipe;
pub mod result;
pub mod scenario;
pub mod step;

pub use error::RecoveryError;
pub use escalation::EscalationPolicy;
pub use event::RecoveryEvent;
pub use recipe::RecoveryRecipe;
pub use result::RecoveryResult;
pub use scenario::FailureScenario;
pub use step::RecoveryStep;
