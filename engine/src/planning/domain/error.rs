//! Planning error types for the Planning Pipeline bounded context.
//!
//! @canonical .pi/architecture/modules/planning-pipeline.md#errors
//! Implements: Contract Freeze — PlanningError enum
//! Issue: issue-contract-freeze
//!
//! All errors use `thiserror` derive macros. No `anyhow` in library code.
//!
//! # Contract (Frozen)
//! - `PlanningError` is the single error type for this module
//! - Each variant carries structured context for error reporting
//! - Implements `std::error::Error` for library compatibility

use thiserror::Error;

/// Errors that can occur during planning pipeline operations.
#[derive(Debug, Error)]
pub enum PlanningError {
    /// The LLM budget was exhausted before planning could complete.
    #[error("LLM budget exhausted: used {used_calls}/{max_calls} calls, {used_tokens}/{max_tokens} tokens")]
    BudgetExhausted {
        /// Number of LLM calls used.
        used_calls: u32,
        /// Maximum number of LLM calls allowed.
        max_calls: u32,
        /// Number of LLM tokens used.
        used_tokens: u32,
        /// Maximum number of LLM tokens allowed.
        max_tokens: u32,
    },

    /// No template could be matched to the user intent.
    #[error("No matching template found for intent: {intent_preview}")]
    NoMatchingTemplate {
        /// Preview of the user's intent (truncated for error messages).
        intent_preview: String,
        /// Number of templates that were evaluated.
        templates_evaluated: u32,
    },

    /// A required parameter is missing after extraction.
    #[error("Missing required parameter '{parameter}' for template '{template_id}': {description}")]
    MissingParameter {
        /// The template that requires this parameter.
        template_id: String,
        /// The name of the missing parameter.
        parameter: String,
        /// Human-readable description of what this parameter is for.
        description: String,
    },

    /// The generated TaskGraph failed validation.
    #[error("Plan validation failed: {detail}")]
    ValidationFailed {
        /// Details about the validation failure.
        detail: String,
        /// Number of validation errors found.
        error_count: u32,
    },

    /// An LLM classification/extraction call failed.
    #[error("Classification error: {detail}")]
    ClassificationError {
        /// Details about the classification error.
        detail: String,
    },

    /// An LLM parameter extraction call failed.
    #[error("Parameter extraction error: {detail}")]
    ExtractionError {
        /// Details about the extraction error.
        detail: String,
    },

    /// The planning pipeline encountered an invalid state.
    #[error("Invalid planning state: {detail}")]
    InvalidState {
        /// Details about the invalid state.
        detail: String,
        /// The phase of the pipeline where the error occurred.
        phase: String,
    },

    /// A repository operation failed.
    #[error("Repository error: {detail}")]
    RepositoryError {
        /// Details about the repository failure.
        detail: String,
    },

    /// The template engine returned an error.
    #[error("Template engine error: {detail}")]
    TemplateEngineError {
        /// Details about the template engine failure.
        detail: String,
    },

    /// A downstream component (DAG engine, validator) returned an error.
    #[error("Downstream component error from {component}: {detail}")]
    DownstreamError {
        /// The component that returned the error.
        component: String,
        /// Details about the error.
        detail: String,
    },

    /// The user cancelled the planning operation.
    #[error("Planning cancelled by user")]
    Cancelled,
}

impl From<crate::template_generation::domain::GeneratorError> for PlanningError {
    fn from(err: crate::template_generation::domain::GeneratorError) -> Self {
        use crate::template_generation::domain::GeneratorError;
        match err {
            GeneratorError::BudgetExhausted {
                calls_used,
                max_calls,
            } => PlanningError::BudgetExhausted {
                used_calls: calls_used,
                max_calls,
                used_tokens: 0,
                max_tokens: 0,
            },
            _ => PlanningError::TemplateEngineError {
                detail: err.to_string(),
            },
        }
    }
}

impl PlanningError {
    /// Returns `true` if this error is transient and the operation may succeed on retry.
    pub fn is_retriable(&self) -> bool {
        matches!(
            self,
            PlanningError::ClassificationError { .. }
                | PlanningError::ExtractionError { .. }
                | PlanningError::TemplateEngineError { .. }
        )
    }
}
