//! Plan Validation — Self-correcting template validation loop.
//!
//! @canonical .pi/architecture/modules/plan-validation.md
//! Implements: Contract Freeze — ValidationLoopConfig, ValidationState, ValidationOutcome,
//! ValidationReport, ValidationLoopService, ContextAugmenter
//! Issue: issue-contract-freeze
//!
//! Bounded context for the self-correcting plan→execute→verify→fix loop:
//!
//! 1. **Plan** — Generate a template from user intent (via PlanningPipeline)
//! 2. **Execute** — Run the template (via ExecutionEngine)
//! 3. **Verify** — Evaluate against quality gates (via QualityGateService)
//! 4. **Fix** — Parse failures, augment context, retry only LLM steps (selective retry)
//!
//! # Core Rule
//!
//! **Deterministic steps are never retried; only `llm_generate` nodes are retried
//! with augmented context.** This preserves the reusability of the template
//! infrastructure while enabling self-correction of generative content.
//!
//! # Architecture (Clean Architecture)
//!
//! ```text
//! plan_validation/
//! ├── domain/                       # Domain entities and interfaces (frozen contracts)
//! │   ├── mod.rs
//! │   ├── loop_config.rs            # ValidationLoopConfig
//! │   ├── state.rs                  # ValidationState
//! │   ├── outcome.rs                # ValidationOutcome
//! │   ├── report.rs                 # ValidationReport, ValidationIterationReport
//! │   ├── error.rs                  # ValidationLoopError enum
//! │   └── event/                    # ValidationEvent payload schemas
//! ├── application/                  # Service traits, DTOs, ContextAugmenter
//! │   ├── service.rs                # ValidationLoopService trait
//! │   ├── context_augmenter.rs       # ContextAugmenter — failure context formatting
//! │   └── dto/                      # Input/output DTOs for all operations
//! ├── infrastructure/               # Repository interfaces
//! │   └── repository/               # ValidationReportRepository, ValidatedTemplateRepository
//! └── interfaces/                   # API contracts
//!     └── http/                     # REST endpoint contracts
//! ```

pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod interfaces;
