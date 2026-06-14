//! Template error types.
//!
//! @canonical .pi/architecture/modules/template-system.md#errors
//! Implements: Contract Freeze — TemplateError enum
//! Issue: #101
//!
//! All errors use `thiserror` derive macros. No `anyhow` in library code.
//!
//! # Contract (Frozen)
//! - `TemplateError` is the single error type for this module
//! - Each variant carries structured context for error reporting
//! - Implements `std::error::Error` for library compatibility
//! - Converted to `CoreOrchestratorError` via `#[from]` at the orchestrator level

use thiserror::Error;

/// Errors that can occur during template parsing, validation, and engine operations.
#[derive(Debug, Error)]
pub enum TemplateError {
    /// Failed to parse TOML content into a Template struct.
    #[error("Failed to parse template: {detail}")]
    Parse {
        /// Human-readable parse error description.
        detail: String,
        /// Source line number if available.
        line: Option<u32>,
        /// Path to the file that failed to parse, if applicable.
        path: Option<String>,
    },

    /// A template required parameter was not provided.
    #[error("Template '{template}' requires parameter '{param}'")]
    MissingParameter {
        /// The template that requires the parameter.
        template: String,
        /// The name of the missing parameter.
        param: String,
        /// Description of the parameter for error context.
        description: Option<String>,
    },

    /// Template not found in the engine registry.
    #[error("Template not found: {id}")]
    NotFound {
        /// The template ID that was requested.
        id: String,
        /// Available template IDs for user guidance.
        available: Vec<String>,
    },

    /// A parameter value has an invalid type.
    #[error("Invalid parameter '{param}': expected {expected}, got {actual}")]
    InvalidParameter {
        /// The parameter name.
        param: String,
        /// Expected type.
        expected: String,
        /// Actual type received.
        actual: String,
        /// Value that caused the error, if representable.
        value: Option<String>,
    },

    /// Template validation failed (structural or semantic).
    #[error("Template validation failed: {field}: {reason}")]
    ValidationFailed {
        /// The field that failed validation.
        field: String,
        /// Why the validation failed.
        reason: String,
        /// The invalid value, if representable.
        value: Option<String>,
    },

    /// A template with the same ID is already registered.
    #[error("Template already registered: {id}")]
    DuplicateTemplate {
        /// The duplicate template ID.
        id: String,
    },

    /// IO error when reading template files.
    #[error("IO error reading template: {io_error}")]
    Io {
        /// The underlying IO error.
        #[from]
        io_error: std::io::Error,
    },

    /// Cycle detected in template node dependencies.
    #[error("Cycle detected in template '{template}': {nodes:?}")]
    CycleDetected {
        /// The template containing the cycle.
        template: String,
        /// Nodes involved in the cycle.
        nodes: Vec<String>,
    },

    /// A dependency reference in a template node is invalid.
    #[error("Dependency not found in template '{template}': {dependency}")]
    DependencyNotFound {
        /// The template containing the invalid reference.
        template: String,
        /// The dependency node ID that was not found.
        dependency: String,
    },

    /// Template generation from an LLM failed.
    #[error("Template generation failed: {detail}")]
    GenerationFailed {
        /// Human-readable description of the failure.
        detail: String,
        /// Number of retry attempts made.
        attempts: u8,
    },
}
