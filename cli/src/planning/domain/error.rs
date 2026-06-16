//! CLI-specific planning pipeline errors.
//!
//! @canonical .pi/architecture/modules/planning-pipeline.md
//! Implements: Contract Freeze — PlanningCliError
//! Issue: issue-contract-freeze

use thiserror::Error;

#[derive(Debug, Error)]
pub enum PlanningCliError {
    #[error("Planning failed: {detail}")]
    PlanningFailed { detail: String },

    #[error("No matching template found for intent")]
    NoTemplateMatch,

    #[error("Intent classification failed: {detail}")]
    ClassificationFailed { detail: String },

    #[error("Parameter extraction failed: {detail}")]
    ExtractionFailed { detail: String },

    #[error("Graph generation failed: {detail}")]
    GraphGenerationFailed { detail: String },

    #[error("Plan validation failed: {errors:?}")]
    ValidationFailed { errors: Vec<String> },

    #[error("Budget check failed: {detail}")]
    BudgetExceeded { detail: String },

    #[error("Internal error: {detail}")]
    Internal { detail: String },
}

impl PlanningCliError {
    pub fn is_retriable(&self) -> bool {
        matches!(
            self,
            PlanningCliError::PlanningFailed { .. } | PlanningCliError::ClassificationFailed { .. }
        )
    }
}
