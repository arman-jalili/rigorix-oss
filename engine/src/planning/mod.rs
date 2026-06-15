//! Planning Pipeline — Orchestrates LLM-based planning from user intent to validated plan.
//!
//! @canonical .pi/architecture/modules/planning-pipeline.md
//! Implements: Contract Freeze — PlanningPipeline and Classifier contracts
//! Issue: issue-contract-freeze
//!
//! Bounded context for the 6-phase planning flow:
//!
//! 1. **Budget Pre-check** — Ensures ≥2 LLM calls remain
//! 2. **Intent Classification** — Matches user intent to template via LLM
//! 3. **Parameter Extraction** — Extracts structured parameters for the matched template
//! 4. **Graph Generation** — Generates TaskGraph from template + parameters
//! 5. **Plan Validation** — Validates the generated plan via CompositeValidator
//! 6. **Hash Computation** — Deterministic planning_hash for replay auditing
//!
//! # Architecture (Clean Architecture)
//!
//! ```text
//! planning/
//! ├── domain/                      # Domain entities and interfaces (frozen contracts)
//! │   ├── mod.rs
//! │   ├── intent.rs                # UserIntent value object
//! │   ├── result.rs                # PlanningResult, PlanningHash, PlanOutput
//! │   ├── classification.rs        # ClassificationResult, Classifier trait
//! │   ├── extractor.rs             # ParameterExtractor trait
//! │   ├── error.rs                 # PlanningError enum
//! │   └── event/                   # PlanningEvent payload schemas
//! ├── application/                 # Service traits, DTOs, factory interfaces
//! │   ├── service.rs               # PlanningPipelineService trait
//! │   ├── factory.rs               # PlanningPipelineFactory trait
//! │   └── dto/                     # Input/output DTOs for all operations
//! ├── infrastructure/              # Repository interfaces
//! │   └── repository/              # PlanningResultRepository trait
//! └── interfaces/                  # API contracts
//!     └── http/                    # REST endpoint contracts
//! ```

pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod interfaces;

#[cfg(test)]
pub(crate) mod tests;

#[cfg(feature = "live-tests")]
pub(crate) mod live_classifier_tests;
