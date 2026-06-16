//! Templates domain types — error types, event schemas for CLI template operations.
//!
//! @canonical .pi/architecture/modules/templates.md
//! Implements: Contract Freeze — TemplateCliError, TemplateCliEvent
//! Issue: issue-contract-freeze
//!
//! These types define the domain-level contracts for CLI template operations.
//! They are pure domain objects with no framework dependencies.
//!
//! # Contract (Frozen)
//! - Errors carry structured context for diagnostics
//! - Events are serializable for logging and CI/CD output
//! - No implementation logic — only type definitions

pub mod error;
pub mod event;

pub use error::TemplateCliError;
pub use event::TemplateCliEvent;
