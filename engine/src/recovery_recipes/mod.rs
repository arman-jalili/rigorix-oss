//! Recovery Recipes bounded context.
//!
//! @canonical .pi/architecture/modules/recovery-recipes.md
//! Implements: Contract Freeze — recovery-recipes module
//! Issue: #438 (recovery-recipes epic)
//!
//! This module encodes known failure scenarios and their automatic recovery
//! procedures. The core rule is: **one automatic recovery attempt per scenario
//! before human escalation.** Each failure scenario has a `RecoveryRecipe` —
//! a sequence of recovery steps with a maximum attempt count and an escalation
//! policy for when attempts are exhausted.
//!
//! # Architecture
//!
//! ```text
//! recovery_recipes/
//! ├── domain/               # Domain entities (FailureScenario, RecoveryStep, RecoveryRecipe,
//! │   │                     #   EscalationPolicy, RecoveryResult, RecoveryEvent, RecoveryError)
//! │   ├── error.rs          # RecoveryError enum
//! │   ├── escalation.rs     # EscalationPolicy enum
//! │   ├── event.rs          # RecoveryEvent payload schemas
//! │   ├── recipe.rs         # RecoveryRecipe struct + default catalog
//! │   ├── result.rs         # RecoveryResult enum
//! │   ├── scenario.rs       # FailureScenario enum + FailureType mapping
//! │   └── step.rs           # RecoveryStep enum
//! ├── application/          # Service traits, DTOs, factory interfaces
//! │   ├── context.rs        # RecoveryContext (per-session attempt tracker)
//! │   ├── dto.rs            # Input/Output DTOs with validation
//! │   └── service.rs        # RecoveryService trait
//! ├── infrastructure/       # Repository interfaces
//! │   └── repository.rs     # RecoveryRecipeRepository trait
//! └── interfaces/           # API contracts
//!     └── http.rs           # REST endpoint contracts
//! ```
//!
//! # Contract Freeze Notice
//!
//! ALL files in this module are frozen contracts.
//! - No implementation changes without explicit contract change approval
//! - Implementation PRs MUST reference these interfaces
//! - DTO schemas serve as the canonical data contract

pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod interfaces;
