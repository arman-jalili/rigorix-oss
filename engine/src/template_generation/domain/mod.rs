//! Domain entities and interfaces for the Template Generation bounded context.
//!
//! @canonical .pi/architecture/modules/template-generation.md#domain
//! Implements: Contract Freeze — TemplateGenerator trait, ClaudeTemplateGenerator,
//! GeneratorError, RepoContext, GeneratedTemplate
//! Issue: issue-contract-freeze
//!
//! This module defines the core domain types for LLM-based template generation.
//! These are pure domain objects with no framework dependencies.

pub mod error;
pub mod event;
pub mod generator;

pub use error::*;
pub use generator::*;
