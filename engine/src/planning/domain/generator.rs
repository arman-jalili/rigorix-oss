//! Re-exports from the standalone template_generation module.
//!
//! @canonical .pi/architecture/modules/template-generation.md
//! Issue: issue-contract-freeze
//!
//! Template generation code has been moved to `src/template_generation/domain/generator.rs`.
//! This file re-exports everything for backward compatibility with existing
//! planning module imports.
//!
//! New code should import directly from `crate::template_generation::domain`.

pub use crate::template_generation::domain::generator::{
    ClaudeGeneratorConfig, ClaudeTemplateGenerator, GeneratedTemplate, GeneratedTemplateCost,
    GeneratorError, InvalidSymbolReference, RepoContext, TemplateGenerator,
};
