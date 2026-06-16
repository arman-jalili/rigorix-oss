//! Event payload schemas for the CLI Templates module.
//!
//! @canonical .pi/architecture/modules/templates.md
//! Implements: Contract Freeze — TemplateCliEvent payload schemas
//! Issue: issue-contract-freeze
//!
//! These events are emitted by the CLI templates module whenever templates
//! are listed, shown, or encounter errors. Consumers (output formatters,
//! TUI, loggers) subscribe to these event types.
//!
//! # Contract (Frozen)
//! - Each event carries the full context needed by consumers
//! - No internal implementation details exposed
//! - All events are serializable for logging and CI/CD output

use serde::{Deserialize, Serialize};

/// Events emitted by the CLI Templates module.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TemplateCliEvent {
    /// A template list operation was initiated.
    TemplateListRequested,

    /// A template list operation completed successfully.
    TemplateListCompleted {
        /// Number of templates returned.
        count: usize,
    },

    /// A template show operation was requested for a specific template.
    TemplateShowRequested {
        /// The template ID being shown.
        template_id: String,
    },

    /// A template show operation completed successfully.
    TemplateShowCompleted {
        /// The template ID that was shown.
        template_id: String,
    },

    /// A template operation failed.
    TemplateOperationFailed {
        /// The operation that failed (e.g., "list", "show").
        operation: String,
        /// The template ID involved, if applicable.
        template_id: Option<String>,
        /// Error message.
        error: String,
    },

    /// The template engine handler was initialized.
    TemplateEngineInitialized {
        /// Whether initialization was successful.
        success: bool,
    },
}
