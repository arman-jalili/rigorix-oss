//! CLI-specific template domain errors.
//!
//! @canonical .pi/architecture/modules/templates.md
//! Implements: Contract Freeze — TemplateCliError
//! Issue: issue-contract-freeze
//!
//! Errors that originate from CLI template operations (list/show).
//! These are distinct from the engine's `TemplateError` — they cover
//! CLI-level concerns like initialization failures, configuration issues,
//! and engine passthrough errors.
//!
//! # Contract (Frozen)
//! - `TemplateCliError` is the single error type for the templates module
//! - Each variant carries structured context for error reporting
//! - Implements `std::error::Error` for library compatibility
//! - Maps to `CliError` at the boundary layer

use thiserror::Error;

/// Errors that can occur during CLI template operations.
#[derive(Debug, Error)]
pub enum TemplateCliError {
    /// The template engine service failed to initialize.
    #[error("Failed to initialize template engine: {detail}")]
    InitializationFailed {
        /// What went wrong during initialization.
        detail: String,
    },

    /// A template with the given ID was not found.
    #[error("Template not found: {template_id}")]
    TemplateNotFound {
        /// The template ID that was not found.
        template_id: String,
    },

    /// Failed to list templates from the engine.
    #[error("Failed to list templates: {detail}")]
    ListFailed {
        /// Details of the failure.
        detail: String,
    },

    /// Failed to show a template's details.
    #[error("Failed to show template '{template_id}': {detail}")]
    ShowFailed {
        /// The template ID being shown.
        template_id: String,
        /// Details of the failure.
        detail: String,
    },

    /// Configuration required for template operations is missing.
    #[error("Template configuration error: {detail}")]
    ConfigError {
        /// Description of the missing or invalid configuration.
        detail: String,
    },

    /// An unexpected internal error occurred.
    #[error("Internal template error: {detail}")]
    Internal {
        /// Description of the internal error.
        detail: String,
    },
}

impl TemplateCliError {
    /// Returns `true` if this error is retriable (user can retry the operation).
    pub fn is_retriable(&self) -> bool {
        matches!(
            self,
            TemplateCliError::ConfigError { .. } | TemplateCliError::TemplateNotFound { .. }
        )
    }
}
