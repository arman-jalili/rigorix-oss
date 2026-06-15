//! Risk gating error types.
//!
//! @canonical .pi/architecture/modules/risk-gating.md
//! Implements: Contract Freeze — RiskGatingError enum
//! Issue: issue-contract-freeze
//!
//! All errors use `thiserror` derive macros. No `anyhow` in library code.
//!
//! # Contract (Frozen)
//! - `RiskGatingError` is the single error type for this module
//! - Each variant carries structured context for error reporting
//! - Implements `std::error::Error` for library compatibility

use thiserror::Error;

/// Errors that can occur during risk gating operations.
#[derive(Debug, Error)]
pub enum RiskGatingError {
    /// A tool name was not recognized by the classifier.
    #[error("Unknown tool: {tool}. No classification rule or override found.")]
    UnknownTool {
        /// The name of the tool that could not be classified.
        tool: String,
    },

    /// Invalid configuration for risk gating.
    #[error("Invalid risk gating configuration: {detail}")]
    InvalidConfiguration {
        /// Details about the configuration error.
        detail: String,
    },

    /// A risk level override value was invalid.
    #[error("Invalid risk level override for tool '{tool}': {value}")]
    InvalidOverride {
        /// The tool with an invalid override.
        tool: String,
        /// The invalid value.
        value: String,
    },

    /// The gate is in an invalid state for the requested operation.
    #[error("Invalid gate state: {detail}")]
    InvalidState {
        /// Details about the state error.
        detail: String,
    },

    /// A classification rule produced an unexpected result.
    #[error("Classification error for tool '{tool}': {detail}")]
    ClassificationError {
        /// The tool being classified.
        tool: String,
        /// Details about the error.
        detail: String,
    },
}

impl RiskGatingError {
    /// Returns `true` if this error is transient and the operation may succeed on retry.
    pub fn is_retriable(&self) -> bool {
        matches!(self, RiskGatingError::ClassificationError { .. })
    }
}
