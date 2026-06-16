//! CLI-specific template generation errors.
//!
//! @canonical .pi/architecture/modules/template-generation.md
//! Implements: Contract Freeze — GenerationCliError
//! Issue: issue-contract-freeze
//!
//! Errors that originate from CLI template generation operations.
//! Distinct from the engine's `GeneratorError`.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum GenerationCliError {
    #[error("Generation failed: {detail}")]
    GenerationFailed { detail: String },

    #[error("Failed to persist generated template: {detail}")]
    PersistFailed { path: String, detail: String },

    #[error("Template validation after generation failed: {errors:?}")]
    ValidationFailed { errors: Vec<String> },

    #[error("LLM budget exceeded for generation: {detail}")]
    BudgetExceeded { detail: String },

    #[error("Repository context build failed: {detail}")]
    RepoContextFailed { detail: String },

    #[error("Internal error: {detail}")]
    Internal { detail: String },
}

impl GenerationCliError {
    pub fn is_retriable(&self) -> bool {
        matches!(self, GenerationCliError::GenerationFailed { .. })
    }
}
