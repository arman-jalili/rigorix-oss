//! Domain entities and interfaces for the Template System bounded context.
//!
//! @canonical .pi/architecture/modules/template-system.md#domain
//! Implements: Contract Freeze — Template, TemplateNode, ParameterDef, TemplateAction,
//!   TemplateError, TemplateEvent
//! Issue: #101
//!
//! This module defines the core domain types — `Template`, `TemplateNode`, `ParameterDef`,
//! `TemplateAction`, and all sub-types. These are pure domain objects with no framework
//! dependencies. They serve as the frozen contract that all implementation must satisfy.
//!
//! # Contract (Frozen)
//! - No implementation logic beyond constructors and field accessors
//! - All validation must happen in the application layer (service traits)
//! - All persistence must happen behind repository interfaces
//! - Template struct is deserialized directly from TOML via serde

pub mod error;
pub mod event;
pub mod template;

pub use error::TemplateError;
pub use template::*;
