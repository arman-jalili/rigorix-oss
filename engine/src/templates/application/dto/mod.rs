//! Data Transfer Objects for the Template System module.
//!
//! @canonical .pi/architecture/modules/template-system.md
//! Implements: Contract Freeze — DTO schemas for parse, register, generate, directory operations
//! Issue: #101
//!
//! DTOs define the input/output contracts for service operations.
//! They carry validation metadata and documentation but no behavior.
//!
//! # Contract (Frozen)
//! - Every service operation has a dedicated input and output DTO
//! - DTOs are serializable (JSON for API)
//! - Validation constraints are documented in field docs
//! - Fields use reasonable Rust types (no framework-specific annotations)

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::templates::domain::{RetryConfig, Template, TemplateAction, ValidationRule};

// ---------------------------------------------------------------------------
// Parse Template DTOs
// ---------------------------------------------------------------------------

/// Input for parsing a template from a TOML file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParseFileInput {
    /// Path to the TOML template file.
    pub path: String,

    /// Whether to validate the parsed template.
    pub validate: bool,
}

/// Input for parsing a template from a TOML string.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParseStrInput {
    /// TOML content to parse.
    pub toml_content: String,

    /// Optional source identifier for error reporting.
    pub source: Option<String>,

    /// Whether to validate the parsed template.
    pub validate: bool,
}

/// Output from parsing a template.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParseOutput {
    /// The parsed template.
    pub template: Template,

    /// Whether validation passed.
    pub valid: bool,

    /// Validation errors encountered (empty if valid).
    pub errors: Vec<String>,

    /// Warnings (non-blocking issues).
    pub warnings: Vec<String>,
}

/// Output from loading a directory of template files.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LoadDirectoryOutput {
    /// Templates successfully parsed.
    pub templates: Vec<Template>,

    /// Templates that failed to parse.
    pub failures: Vec<TemplateLoadFailure>,

    /// Total number of files found.
    pub total_files: usize,

    /// Number of successfully parsed templates.
    pub successful: usize,
}

/// A template file that failed to load.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TemplateLoadFailure {
    /// The file path that failed.
    pub path: String,

    /// Error message.
    pub error: String,

    /// Line number of the error, if available.
    pub line: Option<u32>,
}

// ---------------------------------------------------------------------------
// Register Template DTOs
// ---------------------------------------------------------------------------

/// Input for registering a template in the engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterInput {
    /// The template to register.
    pub template: Template,

    /// Whether to overwrite if a template with the same ID exists.
    pub overwrite: bool,
}

/// Output from registering a template.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RegisterOutput {
    /// The registered template ID.
    pub template_id: String,

    /// Total number of registered templates.
    pub total_templates: usize,

    /// Whether this was an overwrite of an existing template.
    pub overwritten: bool,
}

// ---------------------------------------------------------------------------
// Generate Graph DTOs
// ---------------------------------------------------------------------------

/// Input for generating an executable graph from a template.
///
/// When the DAG Engine module (crate::dag) is implemented, the return type
/// will change to `Result<TaskGraph, TemplateError>`. This DTO serves as
/// the frozen contract boundary until then.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateInput {
    /// The template ID to instantiate.
    pub template_id: String,

    /// Parameter values for `{{ param }}` substitution.
    pub params: HashMap<String, serde_json::Value>,

    /// Execution ID to associate with the generated graph.
    pub execution_id: uuid::Uuid,

    /// Whether to perform topological sort and cycle detection.
    pub validate_graph: bool,
}

/// Output from generating an executable graph.
///
/// @contract This is a temporary DTO that represents the generated graph data.
///   When the DAG Engine module is implemented, the service trait's return type
///   should be updated to `Result<crate::dag::graph::TaskGraph, TemplateError>`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GenerateOutput {
    /// The template ID used for generation.
    pub template_id: String,

    /// Generated nodes with parameters substituted.
    pub nodes: Vec<GeneratedNode>,

    /// Dependency edges as (node_id, depends_on_id) pairs.
    pub edges: Vec<(String, String)>,

    /// Whether the graph passed validation.
    pub valid: bool,

    /// Topological order of node IDs (empty if validation failed).
    pub topological_order: Vec<String>,

    /// Validation errors (empty if valid).
    pub errors: Vec<String>,

    /// The execution ID this graph is associated with.
    pub execution_id: uuid::Uuid,

    /// Total node count in the generated graph.
    pub node_count: usize,
}

/// A single node in a generated graph with resolved parameter values.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GeneratedNode {
    /// Node identifier.
    pub id: String,

    /// Node name.
    pub name: String,

    /// Resolved action with parameter placeholders substituted.
    pub action: TemplateAction,

    /// Resolved retry configuration.
    pub retry: RetryConfig,

    /// Validation rules.
    pub validate: Vec<ValidationRule>,
}

// ---------------------------------------------------------------------------
// Template Query DTOs
// ---------------------------------------------------------------------------

/// Input for looking up a template in the engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetTemplateInput {
    /// The template ID to look up.
    pub template_id: String,
}

/// Output from listing all registered templates.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ListTemplatesOutput {
    /// All registered template metadata (without full node definitions).
    pub templates: Vec<TemplateSummary>,

    /// Total count.
    pub total: usize,
}

/// Lightweight summary of a template for listing operations.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TemplateSummary {
    /// Unique template ID.
    pub id: String,

    /// Human-readable name.
    pub name: String,

    /// Brief description.
    pub description: String,

    /// Version string.
    pub version: String,

    /// Number of parameters.
    pub param_count: usize,

    /// Number of nodes.
    pub node_count: usize,

    /// Tags for categorization.
    pub tags: Vec<String>,

    /// Optional category.
    pub category: Option<String>,

    /// Template version for built-in template identification.
    pub is_builtin: bool,
}

// ---------------------------------------------------------------------------
// Validate Template DTOs
// ---------------------------------------------------------------------------

/// Input for validating a template definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidateTemplateInput {
    /// The template to validate.
    pub template: Template,

    /// Whether to check for cycles in the node dependency graph.
    pub check_cycles: bool,

    /// Whether to validate parameter references in node actions.
    pub check_param_references: bool,
}

/// Output from validating a template.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ValidateTemplateOutput {
    /// Whether all validation checks passed.
    pub valid: bool,

    /// Validation errors (empty if valid).
    pub errors: Vec<ValidationError>,

    /// Warnings (non-blocking issues).
    pub warnings: Vec<String>,

    /// Whether cycle detection was performed and passed.
    pub cycles_checked: bool,

    /// Whether parameter reference validation was performed and passed.
    pub params_checked: bool,
}

/// A single validation error with structured context.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ValidationError {
    /// The field or node that failed validation.
    pub field: String,

    /// Human-readable error message.
    pub message: String,

    /// The invalid value, if representable.
    pub value: Option<String>,

    /// Severity level.
    pub severity: ValidationSeverity,
}

/// Severity level of a validation issue.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ValidationSeverity {
    /// Error — must be fixed for the template to be valid.
    Error,
    /// Warning — should be fixed but doesn't prevent registration.
    Warning,
}

// ---------------------------------------------------------------------------
// Builtin Template DTOs
// ---------------------------------------------------------------------------

/// Input for loading built-in templates.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadBuiltinsInput {
    /// Optional filter to load only specific categories.
    pub categories: Option<Vec<String>>,

    /// Whether to overwrite existing templates with the same IDs.
    pub overwrite: bool,
}

/// Output from loading built-in templates.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LoadBuiltinsOutput {
    /// Templates that were loaded.
    pub loaded: Vec<String>,

    /// Number of templates loaded.
    pub count: usize,
}

// ---------------------------------------------------------------------------
// Template Configuration DTOs
// ---------------------------------------------------------------------------

/// Configuration for the Template System module.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TemplateSystemConfig {
    /// Directories to scan for template files.
    pub template_dirs: Vec<String>,

    /// File extension for template files (default: "toml").
    pub file_extension: String,

    /// Whether to load built-in templates on startup.
    pub load_builtins: bool,

    /// Maximum template file size in bytes.
    pub max_file_size: u64,
}

impl Default for TemplateSystemConfig {
    fn default() -> Self {
        Self {
            template_dirs: vec!["templates".to_string(), ".rigorix/templates".to_string()],
            file_extension: "toml".to_string(),
            load_builtins: true,
            max_file_size: 1_048_576, // 1 MB
        }
    }
}
