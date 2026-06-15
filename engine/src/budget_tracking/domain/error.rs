//! Budget tracking error types.
//!
//! @canonical .pi/architecture/modules/budget-tracking.md#errors
//! @canonical .pi/architecture/decisions/ADR-XXX-error-handling.md
//! Implements: Contract Freeze — LlmBudgetError enum
//! Issue: #68
//!
//! All errors use `thiserror` derive macros. No `anyhow` in library code.
//!
//! # Contract (Frozen)
//! - `LlmBudgetError` is the single error type for this module
//! - Each variant carries structured context for error reporting
//! - Implements `std::error::Error` for library compatibility
//! - Converted to `CoreOrchestratorError` via `#[from]` at the orchestrator level

use thiserror::Error;

/// Errors that can occur during LLM budget operations.
#[derive(Debug, Error)]
pub enum LlmBudgetError {
    /// The maximum number of LLM calls has been exceeded.
    ///
    /// No more calls can be reserved until the budget resets.
    #[error("Max LLM calls exceeded: used {used}/{max}")]
    MaxCallsExceeded {
        /// Number of calls already used.
        used: u32,
        /// Maximum calls allowed.
        max: u32,
    },

    /// The maximum token limit has been exceeded.
    ///
    /// No more tokens can be reserved until the budget resets.
    #[error("Max tokens exceeded: used {used}/{max} (requested {requested})")]
    MaxTokensExceeded {
        /// Number of tokens already used.
        used: u32,
        /// Maximum tokens allowed.
        max: u32,
        /// Number of tokens requested in this reservation.
        requested: u32,
    },

    /// A budget reservation failed for an unspecified reason.
    ///
    /// Catch-all for unexpected reservation failures.
    #[error("Budget reservation failed: {detail}")]
    ReservationFailed {
        /// Human-readable error description.
        detail: String,
        /// Number of tokens that were requested.
        requested_tokens: u32,
    },

    /// The budget has not been initialized or configured.
    #[error("Budget not initialized: {detail}")]
    NotInitialized {
        /// Which configuration field is missing.
        detail: String,
    },

    /// An internal error occurred (e.g. lock poisoned, channel closed).
    #[error("Internal budget error: {detail}")]
    Internal {
        /// Error detail for diagnostics.
        detail: String,
    },
}
impl LlmBudgetError {
    pub fn is_retriable(&self) -> bool {
        false
    }
}
