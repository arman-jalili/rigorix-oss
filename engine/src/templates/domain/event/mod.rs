//! Event payload schemas for the Template System bounded context.
//!
//! @canonical .pi/architecture/decisions/ADR-005-event-bus-persistence.md
//! Implements: Contract Freeze — TemplateEvent payload schemas
//! Issue: #101
//!
//! These events are emitted on the `EventBus` whenever templates are parsed,
//! registered, generated, or encounter errors. Consumers (audit, console printer,
//! TUI) subscribe to these event types.
//!
//! # Contract (Frozen)
//! - Each event carries the full context needed by consumers
//! - No internal implementation details exposed
//! - `sequence` is populated by EventBus at emission time

use serde::{Deserialize, Serialize};

/// Events emitted by the Template System module.
///
/// Wrapped in `ExecutionEvent::TemplateSystem(...)` at the orchestration layer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TemplateEvent {
    /// A template was successfully parsed from a TOML file.
    TemplateParsed {
        /// The template ID.
        template_id: String,
        /// The source file path (if loaded from file).
        source_path: Option<String>,
        /// Number of nodes in the template.
        node_count: usize,
        /// Number of parameters defined.
        param_count: usize,
    },

    /// A template was registered in the TemplateEngine.
    TemplateRegistered {
        /// The template ID.
        template_id: String,
        /// Total number of registered templates after this registration.
        total_templates: usize,
    },

    /// A template was used to generate an executable graph.
    TemplateGenerated {
        /// The template ID used for generation.
        template_id: String,
        /// The execution ID this generation is for.
        execution_id: uuid::Uuid,
        /// Parameters provided for substitution.
        param_keys: Vec<String>,
        /// Number of nodes in the generated graph.
        node_count: usize,
    },

    /// Template parsing failed.
    TemplateParseFailed {
        /// The source path that failed (if applicable).
        source_path: Option<String>,
        /// Error detail for diagnostics.
        error: String,
    },

    /// Template validation encountered an error.
    TemplateValidationFailed {
        /// The template ID being validated.
        template_id: String,
        /// List of validation errors.
        errors: Vec<String>,
    },

    /// A built-in template was loaded successfully.
    BuiltinTemplateLoaded {
        /// The template ID.
        template_id: String,
        /// Template category.
        category: String,
    },

    /// The template directory was scanned and templates were loaded.
    TemplateDirectoryScanned {
        /// The directory path scanned.
        directory: String,
        /// Number of templates found.
        count: usize,
    },
}
