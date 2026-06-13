//! Failure classification error types.
//!
//! @canonical .pi/architecture/modules/error-handling.md#failure-classification
//! Implements: Contract Freeze — FailureClassificationError enum
//! Issue: #33
//!
//! All errors use `thiserror` derive macros. No `anyhow` in library code.
//!
//! # Contract (Frozen)
//! - `FailureClassificationError` is the single error type for this module
//! - Each variant carries structured context for error reporting
//! - Implements `std::error::Error` for library compatibility
//! - Converted to `CoreOrchestratorError` via `#[from]` at the orchestrator level

use thiserror::Error;

/// Errors that can occur during failure classification and strategy selection.
#[derive(Debug, Error)]
pub enum FailureClassificationError {
    /// No matching `FailureType` could be determined for a given error message.
    #[error("Could not classify error message: {reason} (message: {message})")]
    ClassificationFailed {
        /// The error message that could not be classified.
        message: String,
        /// Why classification failed (e.g., "empty message", "no matching pattern").
        reason: String,
    },

    /// No `RetryStrategy` is defined for the given `FailureType`.
    #[error("No retry strategy defined for failure type: {failure_type:?}")]
    MissingStrategy {
        /// The failure type that has no mapping.
        failure_type: String,
    },

    /// The `ExpandContext` strategy has an invalid level value.
    #[error("Invalid expansion level: {level}. Must be between 0 and 5.")]
    InvalidExpansionLevel {
        /// The invalid level value.
        level: u8,
        /// Minimum allowed value.
        min: u8,
        /// Maximum allowed value.
        max: u8,
    },

    /// An invalid or empty error message was provided for classification.
    #[error("Invalid input for classification: {detail}")]
    InvalidInput {
        /// Details about why the input is invalid.
        detail: String,
    },

    /// The pattern repository returned an error during lookup.
    #[error("Pattern repository error: {detail}")]
    PatternRepository {
        /// Details about the repository error.
        detail: String,
    },
}
