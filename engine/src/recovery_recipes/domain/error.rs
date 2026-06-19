//! Recovery error types for the recovery-recipes bounded context.
//!
//! @canonical .pi/architecture/modules/recovery-recipes.md#error-handling
//! Implements: Contract Freeze — RecoveryError enum
//! Issue: #438 (recovery-recipes epic)
//!
//! All errors use `thiserror` derive macros. No `anyhow` in library code.
//!
//! # Contract (Frozen)
//! - `RecoveryError` is the single error type for this module
//! - Each variant carries structured context for error reporting
//! - Implements `std::error::Error` for library compatibility
//! - Converted to `CoreOrchestratorError` via `#[from]` at the orchestrator level

use thiserror::Error;

use super::scenario::FailureScenario;
use super::step::RecoveryStep;

/// Errors that can occur during recovery recipe execution.
#[derive(Debug, Error)]
pub enum RecoveryError {
    /// No recipe is configured for the given scenario.
    #[error("No recipe for scenario: {0:?}")]
    NoRecipe(FailureScenario),

    /// Maximum automatic recovery attempts have been reached.
    #[error("Max recovery attempts reached for {0:?}")]
    MaxAttemptsReached(FailureScenario),

    /// A specific recovery step failed during execution.
    #[error("Recovery step failed: {step:?} — {reason}")]
    StepFailed {
        /// The step that failed.
        step: RecoveryStep,
        /// Human-readable description of the failure.
        reason: String,
    },

    /// Recovery was aborted by a cancellation signal.
    #[error("Recovery aborted by cancellation signal")]
    Aborted,

    /// The recipe configuration is invalid.
    #[error("Invalid recipe configuration: {detail}")]
    InvalidConfiguration {
        /// Details about why the configuration is invalid.
        detail: String,
    },

    /// A required dependency (event bus, service, etc.) is unavailable.
    #[error("Dependency unavailable: {dependency} — {reason}")]
    DependencyUnavailable {
        /// Name of the unavailable dependency.
        dependency: String,
        /// Details about the failure.
        reason: String,
    },
}

impl RecoveryError {
    /// Returns `true` if this error represents a transient condition
    /// that might succeed on retry.
    pub fn is_retriable(&self) -> bool {
        matches!(
            self,
            RecoveryError::Aborted | RecoveryError::DependencyUnavailable { .. }
        )
    }
}
