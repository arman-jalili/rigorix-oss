//! Template Tools — Bridges MCP tool calls to template filesystem.
//!
//! @canonical .pi/architecture/modules/template-tools.md
//! Implements: Contract Freeze — template-tools module root
//!
//! Template Tools bridges MCP tool calls to template filesystem operations:
//! discover templates (`rigorix_list_templates`), read templates
//! (`rigorix_get_template`), create templates (`rigorix_create_template`),
//! and validate template structure (`rigorix_validate_template`).
//!
//! Templates are stored as TOML files in `.rigorix/templates/` directory,
//! making them portable across AI tools.
//!
//! # Module Structure
//!
//! This module follows Clean Architecture with bounded context (DDD):
//!
//! - `template-tools/domain/` — TemplateRepository trait (aggregate root),
//!   TemplateConverter trait, PlanTemplate value object, TemplateSummary,
//!   TemplateFilter, domain events, error types
//! - `template-tools/application/` — Service traits (ListTemplatesHandler,
//!   GetTemplateHandler, CreateTemplateHandler, ValidateTemplateHandler),
//!   input/output DTOs, factory interfaces
//! - `template-tools/infrastructure/` — Repository interface for template
//!   filesystem persistence
//! - `template-tools/interfaces/` — MCP tool handler contracts and schemas
//!
//! # Architecture References
//!
//! | Component | Path (per architecture) | Canonical Section |
//! |-----------|------------------------|-------------------|
//! | TemplateRepository (aggregate root) | `src/template-tools/domain/entity.rs` | `.pi/architecture/modules/template-tools.md#templaterepository` |
//! | TemplateConverter (domain service) | `src/template-tools/domain/entity.rs` | `.pi/architecture/modules/template-tools.md#templateconverter` |
//! | ListTemplatesHandler (domain service) | `src/template-tools/application/service.rs` | `.pi/architecture/modules/template-tools.md#listtemplateshandler` |
//! | GetTemplateHandler (domain service) | `src/template-tools/application/service.rs` | `.pi/architecture/modules/template-tools.md#gettemplatehandler` |
//! | CreateTemplateHandler (domain service) | `src/template-tools/application/service.rs` | `.pi/architecture/modules/template-tools.md#createtemplatehandler` |
//! | ValidateTemplateHandler (domain service) | `src/template-tools/application/service.rs` | `.pi/architecture/modules/template-tools.md#validatetemplatehandler` |
//!
//! # Dependencies
//!
//! - **Depends on:** MCP Server (via ToolRegistry registration), Execution Tools
//!   (shares EngineFacade trait for template validation against enforcement policies)
//! - **Used by:** None directly (leaf handler)
//!
//! # Contract (Frozen)
//!
//! - All public interfaces are frozen — no additions without ADR approval
//! - Domain types are pure data with serde Serialize/Deserialize
//! - Service traits are async (async-trait) and return domain error types
//! - Repository interfaces abstract all persistence concerns
//! - MCP tool handler contracts are framework-agnostic

pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod interfaces;
