//! Template Generation — LLM-based template generation from user intent.
//!
//! @canonical .pi/architecture/modules/template-generation.md
//! Implements: Contract Freeze — TemplateGenerator trait, ClaudeTemplateGenerator,
//! GeneratorError, RepoContext
//! Issue: issue-contract-freeze
//!
//! Generates new TOML workflow templates from natural language user intent when
//! no matching template exists. Plugs into PlanningPipeline as a fallback between
//! classifier and template engine.
//!
//! # Architecture
//!
//! - `domain/`: Core entities (TemplateGenerator trait, ClaudeTemplateGenerator,
//!   RepoContext, GeneratorError, GeneratedTemplate)
//! - `application/`: Service traits, DTOs, symbol validation implementation
//! - `infrastructure/`: Repository interfaces for generated template persistence
//! - `interfaces/`: HTTP API contracts

pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod interfaces;

#[cfg(test)]
pub(crate) mod tests;

#[cfg(all(test, feature = "live-tests"))]
pub(crate) mod live_generator_tests;
