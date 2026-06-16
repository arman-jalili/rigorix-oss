//! Templates module — template list/show command handler.
//!
//! @canonical .pi/architecture/modules/templates.md
//! Implements: Contract Freeze — CLI Templates module (interfaces only)
//! Issue: issue-contract-freeze
//!
//! Wraps the engine's TemplateEngineService for CLI consumption.
//! Implements `rigorix template list` and `rigorix template show`.
//!
//! # Architecture (Clean Architecture layers)
//!
//! ```text
//! templates/
//! ├── domain/           # TemplateCliError, TemplateCliEvent
//! │   ├── mod.rs
//! │   ├── error.rs      # TemplateCliError enum
//! │   └── event/        # TemplateCliEvent payload schemas
//! │       └── mod.rs
//! ├── application/      # Service traits, DTO schemas
//! │   ├── mod.rs
//! │   ├── service.rs    # TemplateCommandService trait
//! │   └── dto/          # TemplateListInput/Output, TemplateShowInput/Output
//! │       └── mod.rs
//! ├── infrastructure/   # Trait implementations, repository interfaces
//! │   ├── mod.rs
//! │   ├── service.rs                    # Re-exports TemplateCommandService
//! │   ├── template_handler_impl.rs       # TemplateEngineHandler impl
//! │   └── repository/                   # TemplateCliRepository trait
//! │       └── mod.rs
//! └── interfaces/       # HTTP API contracts
//!     ├── mod.rs
//!     └── http/         # Endpoint definitions, request/response schemas
//!         └── mod.rs
//! ```
//!
//! # Contract Freeze Notice
//!
//! ALL interface files in this module are frozen contracts.
//! - No implementation changes without explicit contract change approval
//! - Implementation PRs MUST reference these interfaces
//! - DTO schemas serve as the canonical data contract
//! - The TemplateCommandService trait is the sole service contract

pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod interfaces;
