//! Planning Pipeline module — CLI wrapper for the 6-phase planning flow.
//!
//! @canonical .pi/architecture/modules/planning-pipeline.md
//! Implements: Contract Freeze — CLI Planning module (interfaces only)
//! Issue: issue-contract-freeze
//!
//! Wraps the engine's PlanningPipelineService for CLI consumption.
//!
//! # Architecture
//!
//! ```text
//! planning/
//! ├── domain/           # PlanningCliError, PlanningCliEvent
//! │   ├── mod.rs
//! │   ├── error.rs      # PlanningCliError enum
//! │   └── event/        # PlanningCliEvent payload schemas
//! │       └── mod.rs
//! ├── application/      # Service traits, DTO schemas
//! │   ├── mod.rs
//! │   ├── service.rs    # PlanCommandService trait
//! │   └── dto/          # PlanInput/Output, Classification types
//! │       └── mod.rs
//! ├── infrastructure/   # Repository interfaces
//! │   ├── mod.rs
//! │   └── repository/   # PlanningRepository trait
//! │       └── mod.rs
//! └── interfaces/       # HTTP API contracts
//!     ├── mod.rs
//!     └── http/         # Endpoint definitions
//!         └── mod.rs
//! ```

pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod interfaces;
