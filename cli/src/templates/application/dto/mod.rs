//! Data Transfer Objects for the CLI Templates module.
//!
//! @canonical .pi/architecture/modules/templates.md
//! Implements: Contract Freeze — CLI template DTO schemas
//! Issue: issue-contract-freeze
//!
//! DTOs define the input/output contracts for CLI template operations
//! (list/show). They are used by the `TemplateCommandService` trait and
//! are mapped to/from `cli_boundary::application::dto` at the boundary.
//!
//! # Contract (Frozen)
//! - Every service operation has a dedicated input and output DTO
//! - DTOs are serializable (JSON for CI/CD output)
//! - Validation constraints are documented in field docs
//! - Fields use reasonable Rust types (no framework-specific annotations)

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Template List DTOs
// ---------------------------------------------------------------------------

/// Input for the template list command.
///
/// Lists all registered templates with summary metadata.
/// Currently takes no parameters — all templates are listed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateListInput {
    // No fields needed — lists all available templates
}

/// Summary of a registered template for list output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateSummary {
    /// The template identifier (e.g., "read-file", "git-commit").
    pub id: String,

    /// Human-readable name (e.g., "Read File", "Git Commit").
    pub name: String,

    /// One-line description of the template's purpose.
    pub description: String,

    /// Whether this is a built-in template that ships with Rigorix.
    pub built_in: bool,
}

/// Output from the template list command.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateListOutput {
    /// All registered template summaries.
    pub templates: Vec<TemplateSummary>,

    /// Total count of templates in the list.
    pub total: u32,
}

// ---------------------------------------------------------------------------
// Template Show DTOs
// ---------------------------------------------------------------------------

/// Input for the template show command.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateShowInput {
    /// The template ID to show.
    pub template_id: String,
}

/// Output from the template show command.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateShowOutput {
    /// The template TOML content rendered as a string.
    ///
    /// This is a human-readable representation of the template definition,
    /// including metadata, parameters, and node definitions.
    pub content: String,
}

// ---------------------------------------------------------------------------
// Conversions to CLI boundary DTOs (for dispatch_command integration)
// ---------------------------------------------------------------------------

impl From<TemplateSummary> for crate::cli_boundary::application::dto::TemplateSummary {
    fn from(s: TemplateSummary) -> Self {
        Self {
            id: s.id,
            name: s.name,
            description: s.description,
            built_in: s.built_in,
        }
    }
}

impl From<TemplateListOutput> for crate::cli_boundary::application::dto::TemplateListOutput {
    fn from(o: TemplateListOutput) -> Self {
        Self {
            templates: o.templates.into_iter().map(Into::into).collect(),
            total: o.total,
        }
    }
}

impl From<TemplateShowOutput> for crate::cli_boundary::application::dto::TemplateShowOutput {
    fn from(o: TemplateShowOutput) -> Self {
        Self { content: o.content }
    }
}
