//! Templates infrastructure — TemplateCommandService implementation, repository interfaces.
//!
//! @canonical .pi/architecture/modules/templates.md
//! Implements: Contract Freeze — repository interfaces, TemplateEngineHandler
//! Issue: issue-contract-freeze
//!
//! Infrastructure layer for CLI template operations:
//! - `service.rs` re-exports the `TemplateCommandService` trait from application/
//! - `template_handler_impl.rs` implements the trait via the engine
//! - `repository/` defines persistence interfaces for CLI-level template data

pub mod repository;
pub mod service;
pub mod template_handler_impl;
pub use service::*;
pub use template_handler_impl::*;
