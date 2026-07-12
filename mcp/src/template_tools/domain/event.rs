//! Domain events for the Template Tools bounded context.
//!
//! @canonical .pi/architecture/modules/template-tools.md#events
//! Implements: Contract Freeze — template-tools event payload schemas
//!
//! These events are emitted throughout template lifecycle operations.
//! Consumers (observability, telemetry, audit trail) subscribe to these
//! event types.
//!
//! # Event Catalog
//!
//! | Event | Description | Trigger | Published By |
//! |-------|-------------|---------|-------------|
//! | TemplateCreated | A new template was saved | CreateTemplateHandler on successful write | CreateTemplateHandler |
//! | TemplateRead | A template was read | GetTemplateHandler on successful read | GetTemplateHandler |
//! | TemplateListed | Templates were listed | ListTemplatesHandler on completed list | ListTemplatesHandler |
//! | TemplateValidated | A template was validated | ValidateTemplateHandler after validation | ValidateTemplateHandler |
//!
//! # Contract (Frozen)
//!
//! - Every event carries a template_name and timestamp for correlation
//! - Serialized as tagged union with `#[serde(tag = "type")]`
//! - Events are facts — immutable and append-only
//! - No behavior — pure data

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// All domain events emitted by the Template Tools bounded context.
///
/// Each variant represents a meaningful domain occurrence.
/// Consumers use these events for observability, logging, and metrics.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TemplateToolsEvent {
    /// A new template was created via `rigorix_create_template`.
    TemplateCreated {
        /// Name of the created template.
        template_name: String,
        /// Number of steps in the template.
        step_count: usize,
        /// Whether an existing template was overwritten.
        overwrite: bool,
        /// Timestamp of creation.
        timestamp: DateTime<Utc>,
    },

    /// A template was read via `rigorix_get_template`.
    TemplateRead {
        /// Name of the read template.
        template_name: String,
        /// Response format requested ("json" or "toml").
        format: Option<String>,
        /// Timestamp of the read.
        timestamp: DateTime<Utc>,
    },

    /// Templates were listed via `rigorix_list_templates`.
    TemplateListed {
        /// Filter criteria used (serialized as string).
        filter_criteria: Option<String>,
        /// Number of templates returned.
        result_count: usize,
        /// Timestamp of the list.
        timestamp: DateTime<Utc>,
    },

    /// A template was validated via `rigorix_validate_template`.
    TemplateValidated {
        /// Name of the validated template.
        template_name: String,
        /// Whether the template is valid.
        is_valid: bool,
        /// Validation error messages.
        errors: Vec<String>,
        /// Timestamp of validation.
        timestamp: DateTime<Utc>,
    },
}
