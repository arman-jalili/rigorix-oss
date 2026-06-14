//! Template System — TOML-based workflow template definitions and runtime registry.
//!
//! @canonical .pi/architecture/modules/template-system.md
//! Implements: Contract Freeze — module root for TemplateParser and TemplateEngine
//! Issue: #101
//!
//! # Module Structure
//!
//! This module follows Clean Architecture with bounded contexts (DDD):
//!
//! - `domain/` — `Template`, `TemplateNode`, `ParameterDef`, `TemplateAction`, `TemplateError`, events
//! - `application/` — Service traits (`TemplateParserService`, `TemplateEngineService`),
//!   DTOs for all operations, factory interfaces
//! - `infrastructure/` — Repository interfaces for template storage
//! - `interfaces/` — HTTP API contracts for template CRUD operations
//!
//! # Architecture References
//!
//! | Component | File (per architecture) | Canonical Section |
//! |-----------|------------------------|-------------------|
//! | TemplateParser | `rigorix/src/templates/parser.rs` | `.pi/architecture/modules/template-system.md#parser` |
//! | TemplateEngine | `rigorix/src/templates/parser.rs` | `.pi/architecture/modules/template-system.md#engine` |
//! | BuiltinTemplates | `rigorix/src/templates/builtin.rs` | `.pi/architecture/modules/template-system.md#builtins` |
//! | TemplateNode | `rigorix/src/templates/parser.rs` | `.pi/architecture/modules/template-system.md#node` |
//! | ParameterDef | `rigorix/src/templates/parser.rs` | `.pi/architecture/modules/template-system.md#params` |
//!
//! # Dependencies
//!
//! - **Depends on:** Configuration (template directory paths), Error Handling (error types)
//! - **Used by:** Planning Pipeline (template selection, graph generation),
//!   Template Generation (registers generated templates), DAG Engine (consumes TaskGraph)

pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod interfaces;
