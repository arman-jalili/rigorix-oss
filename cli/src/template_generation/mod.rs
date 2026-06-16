//! Template Generation module — CLI wrapper for LLM-based template creation.
//!
//! @canonical .pi/architecture/modules/template-generation.md
//! Implements: Contract Freeze — CLI Template Generation module (interfaces only)
//! Issue: issue-contract-freeze
//!
//! Wraps the engine's TemplateGenerator for CLI consumption via `rigorix generate`.
//!
//! # Architecture
//!
//! ```text
//! template_generation/
//! ├── domain/           # GenerationCliError, TemplateGenerationCliEvent
//! │   ├── mod.rs
//! │   ├── error.rs      # GenerationCliError enum
//! │   └── event/        # TemplateGenerationCliEvent payload schemas
//! │       └── mod.rs
//! ├── application/      # Service traits, DTO schemas
//! │   ├── mod.rs
//! │   ├── service.rs    # GenerateCommandService trait
//! │   └── dto/          # GenerateInput/Output, DryRunInput/Output
//! │       └── mod.rs
//! ├── infrastructure/   # Repository interfaces
//! │   ├── mod.rs
//! │   └── repository/   # TemplateGenerationRepository trait
//! │       └── mod.rs
//! └── interfaces/       # HTTP API contracts
//!     ├── mod.rs
//!     └── http/         # Endpoint definitions, request/response schemas
//!         └── mod.rs
//! ```

pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod interfaces;
