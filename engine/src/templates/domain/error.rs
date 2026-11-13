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

/// `" in 'path'"` when there is a path, `""` when there is not — never `"None"`.
fn path_suffix(path: Option<&str>) -> String {
    path.map_or_else(String::new, |p| format!(" in '{p}'"))
}

/// `" at line N"` when there is a line, `""` when there is not.
fn line_suffix(line: Option<u32>) -> String {
    line.map_or_else(String::new, |l| format!(" at line {l}"))
}

/// Errors that can occur during template parsing, validation, and engine operations.
#[derive(Debug, Error)]
pub enum TemplateError {
    /// Failed to parse TOML content into a Template struct.
    #[error("Failed to parse template{file}{line}: {detail}",
        file = path_suffix(.path.as_deref()),
        line = line_suffix(*.line))]
    Parse {
        /// Human-readable parse error description.
        detail: String,
        /// Source line number if available.
        line: Option<u32>,
        /// Path to the file that failed to parse, if applicable.
        path: Option<String>,
    },

    /// A `[[nodes]]` document declares no `schema_version` (#1529, ADR-024 §4 D3).
    ///
    /// A document that does not say which schema it is written in cannot be read: a later version's
    /// field would be ignored by `serde(default)` and the document would silently mean something
    /// else. The legacy `[[steps]]` dialect is exempt by definition — it *is* version 1 — and that
    /// exemption is decided by the dialect, not by a per-field default.
    #[error("Template document{file} has no 'schema_version'; add 'schema_version = 1'", file = path_suffix(.path.as_deref()))]
    MissingSchemaVersion {
        /// The file the document came from, when the caller had one.
        path: Option<String>,
    },

    /// A document names a schema version this engine does not read (#1529).
    #[error("Template document{file} declares schema_version = {found}, which this engine does not read (supported: {supported:?}); an unknown version is refused, never coerced", file = path_suffix(.path.as_deref()))]
    UnsupportedSchemaVersion {
        /// The version the document named.
        found: u16,
        /// The versions this engine reads.
        supported: Vec<u16>,
        /// The file the document came from.
        path: Option<String>,
    },

    /// A document carries both `[[nodes]]` and `[[steps]]`, so it says two things (#1529).
    #[error("Template document{file} carries both [[nodes]] and [[steps]]; one document has one dialect", file = path_suffix(.path.as_deref()))]
    AmbiguousDialect {
        /// The file the document came from.
        path: Option<String>,
    },

    /// A node's invocation is not exactly one of the two spellings (#1529).
    #[error("Template node '{node}' in{file} has an invalid invocation: {reason}", file = path_suffix(.path.as_deref()))]
    InvalidInvocation {
        /// The node's name, so the author can find it.
        node: String,
        /// What is wrong.
        reason: String,
        /// The file the document came from.
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
impl TemplateError {
    pub fn is_retriable(&self) -> bool {
        false
    }
}
