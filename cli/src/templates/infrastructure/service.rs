//! Template command service — re-exported from the application layer.
//!
//! @canonical .pi/architecture/modules/templates.md
//! The `TemplateCommandService` trait is defined in `application/service.rs`
//! (its canonical Clean Architecture location). This module re-exports it
//! for backward compatibility with existing imports.
//!
//! # Migration
//! New code should import directly from `crate::templates::application::TemplateCommandService`.
//! This re-export will be removed in a future update.

pub use crate::templates::application::service::TemplateCommandService;
