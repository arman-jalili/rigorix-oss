//! Domain layer for the Template Tools bounded context.
//!
//! @canonical .pi/architecture/modules/template-tools.md#domain
//! Implements: Contract Freeze — TemplateRepository trait, TemplateConverter trait,
//! value objects, events, error types
//!
//! This module defines the core domain types for Template Tools:
//! - Aggregate root: `TemplateRepository` trait
//! - Domain service: `TemplateConverter` trait
//! - Value objects: `PlanTemplate`, `TemplateSummary`, `TemplateFilter`,
//!   `StepDefinition`, `Constraints`, `TemplateName`
//! - Domain events: `TemplateToolsEvent`
//! - Error types: `TemplateError`
//!
//! These are pure domain types with no framework dependencies. They serve as
//! the frozen contract that all implementation must satisfy.
//!
//! # Contract (Frozen)
//!
//! - No implementation logic beyond constructors and field accessors
//! - All types are serializable (Serialize + Deserialize)
//! - Value objects are immutable (no pub fields, no setters)
//! - TemplateRepository exposes trait methods, not pub fields
//! - Domain events carry template_name and timestamp

pub mod entity;
pub mod error;
pub mod event;
pub mod value;

pub use entity::{TemplateConverter, TemplateRepository};
pub use error::TemplateError;
pub use event::TemplateToolsEvent;
pub use value::{
    Constraints, CreateTemplateInput, GetTemplateInput, PlanTemplate, StepDefinition,
    TemplateFilter, TemplateName, TemplateSummary, ValidateTemplateInput,
};
