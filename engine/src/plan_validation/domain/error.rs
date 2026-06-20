//! ValidationLoopError — typed errors for the plan validation loop.
//!
//! @canonical .pi/architecture/modules/plan-validation.md#error
//! Implements: Contract Freeze — ValidationLoopError
//! Issue: issue-contract-freeze
//!
//! Defines the error types that can occur during validation loop
//! execution. Each variant carries structured context for error
//! reporting and recovery.
//!
//! # Contract (Frozen)
//! - `ValidationLoopError` is the single error type for this module
//! - Each variant carries structured context for error reporting
//! - Implements `std::error::Error` for library compatibility
//! - Conversion from downstream error types via `From` impls

use thiserror::Error;

/// Errors that can occur during plan validation loop operations.
#[derive(Debug, Error)]
pub enum ValidationLoopError {
    /// The planning pipeline returned an error during re-planning.
    #[error("Planning error: {0}")]
    PlanningError(#[from] crate::planning::domain::error::PlanningError),

    /// The execution engine returned an error during graph execution.
    #[error("Execution error: {detail}")]
    ExecutionError {
        /// Details about the execution failure.
        detail: String,
    },

    /// The failure parser returned an error.
    #[error("Failure parser error: {0}")]
    FailureParserError(#[from] crate::failure_parser::domain::FailureParserError),

    /// The quality gates service returned an error.
    #[error("Quality gate error: {detail}")]
    QualityGateError {
        /// Details about the quality gate failure.
        detail: String,
    },

    /// The template engine returned an error.
    #[error("Template engine error: {detail}")]
    TemplateEngineError {
        /// Details about the template engine failure.
        detail: String,
    },

    /// Invalid operation or state encountered.
    #[error("Invalid validation state: {detail}")]
    InvalidState {
        /// Details about the invalid state.
        detail: String,
    },

    /// A repository operation failed.
    #[error("Repository error: {detail}")]
    RepositoryError {
        /// Details about the repository failure.
        detail: String,
    },

    /// The validation was cancelled.
    #[error("Validation cancelled: {reason}")]
    Cancelled {
        /// Reason for cancellation, if known.
        reason: String,
    },
}

impl ValidationLoopError {
    /// Returns `true` if this error is transient and the operation may succeed on retry.
    pub fn is_retriable(&self) -> bool {
        matches!(
            self,
            ValidationLoopError::PlanningError { .. } | ValidationLoopError::ExecutionError { .. }
        )
    }
}
