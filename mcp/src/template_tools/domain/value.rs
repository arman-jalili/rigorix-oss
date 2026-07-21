//! Value objects for the Template Tools bounded context.
//!
//! @canonical .pi/architecture/modules/template-tools.md#value-objects
//! Implements: Contract Freeze — PlanTemplate, TemplateSummary, TemplateFilter,
//! StepDefinition, Constraints, GetTemplateInput, CreateTemplateInput,
//! ValidateTemplateInput, TemplateName
//!
//! Value objects are immutable, interchangeable, and defined by their attributes,
//! not identity. They carry validation in their constructors and are serializable
//! for API transmission.
//!
//! # Contract (Frozen)
//!
//! - All value objects are immutable (no pub fields, no setters)
//! - All value objects implement PartialEq
//! - Constructors validate invariants — return Result<_, Error> on failure
//! - All types derive Serialize + Deserialize for JSON/TOML transmission
//! - No behavior beyond field accessors and validation

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::error::TemplateError;

// ---------------------------------------------------------------------------
// TemplateName — validated template identifier
// ---------------------------------------------------------------------------

/// A validated template name that is filesystem-safe.
///
/// Only allows `[a-zA-Z0-9_-]` characters to prevent path traversal
/// and ensure cross-platform compatibility.
///
/// # Contract (Frozen)
///
/// - Validated on construction — invalid names return error
/// - Input must contain only alphanumeric, underscore, and hyphen chars
/// - Length between 1 and 255 characters
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct TemplateName(String);

impl TemplateName {
    /// Create a new validated TemplateName.
    ///
    /// # Errors
    /// Returns `TemplateError::InvalidName` if the name contains invalid
    /// characters or exceeds the maximum length.
    pub fn new(name: impl Into<String>) -> Result<Self, TemplateError> {
        let name = name.into();
        if name.is_empty() {
            return Err(TemplateError::InvalidName(
                "Template name cannot be empty".into(),
            ));
        }
        if name.len() > 255 {
            return Err(TemplateError::InvalidName(
                "Template name must be 255 characters or fewer".into(),
            ));
        }
        if !name
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
        {
            return Err(TemplateError::InvalidName(
                "Template name must only contain alphanumeric, underscore, and hyphen characters"
                    .into(),
            ));
        }
        Ok(Self(name))
    }

    /// Returns the inner string value.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for TemplateName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<TemplateName> for String {
    fn from(name: TemplateName) -> Self {
        name.0
    }
}

impl TryFrom<String> for TemplateName {
    type Error = TemplateError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

// ---------------------------------------------------------------------------
// PlanTemplate — structured template for plans
// ---------------------------------------------------------------------------

/// A structured template for plans stored as TOML files.
///
/// Represents a template that can be used to scaffold plans. Includes
/// metadata, versioning, and lifecycle timestamps.
///
/// # Contract (Frozen)
///
/// - Must have at least one step
/// - Step order is significant
/// - Constraints are optional enforcement boundaries
/// - Metadata is opaque key-value storage for extensibility
/// - Timestamps use UTC timezone
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlanTemplate {
    /// Name identifying the template.
    #[serde(rename = "name")]
    name: String,

    /// Human-readable description of the template's purpose.
    #[serde(rename = "description")]
    description: String,

    /// Template version (semver format recommended).
    #[serde(rename = "version", default)]
    version: String,

    /// Tags for categorization and filtering.
    #[serde(rename = "tags", default)]
    tags: Vec<String>,

    /// Ordered list of steps in the template.
    #[serde(rename = "steps")]
    steps: Vec<StepDefinition>,

    /// Optional enforcement constraints.
    #[serde(rename = "constraints", skip_serializing_if = "Option::is_none")]
    constraints: Option<Constraints>,

    /// Extensible metadata (e.g., source, author, session context).
    #[serde(rename = "metadata", default)]
    metadata: HashMap<String, String>,

    /// Timestamp when the template was created.
    #[serde(rename = "created_at")]
    created_at: DateTime<Utc>,

    /// Timestamp when the template was last updated.
    #[serde(rename = "updated_at")]
    updated_at: DateTime<Utc>,
}

impl PlanTemplate {
    /// Create a new PlanTemplate with validation.
    ///
    /// # Errors
    /// Returns `TemplateError::ValidationError` if `steps` is empty.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        name: String,
        description: String,
        version: String,
        tags: Vec<String>,
        steps: Vec<StepDefinition>,
        constraints: Option<Constraints>,
        metadata: HashMap<String, String>,
        created_at: DateTime<Utc>,
        updated_at: DateTime<Utc>,
    ) -> Result<Self, TemplateError> {
        if steps.is_empty() {
            return Err(TemplateError::ValidationError(
                "Template must have at least one step".into(),
            ));
        }
        Ok(Self {
            name,
            description,
            version,
            tags,
            steps,
            constraints,
            metadata,
            created_at,
            updated_at,
        })
    }

    /// Create a PlanTemplate from a JSON value.
    ///
    /// Deserializes and validates the template in one step.
    pub fn from_json(value: serde_json::Value) -> Result<Self, TemplateError> {
        let template: Self = serde_json::from_value(value).map_err(|e| {
            TemplateError::DeserializationFailed(format!(
                "Failed to deserialize PlanTemplate from JSON: {}",
                e
            ))
        })?;
        if template.steps.is_empty() {
            return Err(TemplateError::ValidationError(
                "Template must have at least one step".into(),
            ));
        }
        Ok(template)
    }

    /// Template name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Template description.
    pub fn description(&self) -> &str {
        &self.description
    }

    /// Template version.
    pub fn version(&self) -> &str {
        &self.version
    }

    /// Tags for categorization.
    pub fn tags(&self) -> &[String] {
        &self.tags
    }

    /// Ordered list of steps.
    pub fn steps(&self) -> &[StepDefinition] {
        &self.steps
    }

    /// Optional enforcement constraints.
    pub fn constraints(&self) -> Option<&Constraints> {
        self.constraints.as_ref()
    }

    /// Extensible metadata.
    pub fn metadata(&self) -> &HashMap<String, String> {
        &self.metadata
    }

    /// Creation timestamp (UTC).
    pub fn created_at(&self) -> &DateTime<Utc> {
        &self.created_at
    }

    /// Last update timestamp (UTC).
    pub fn updated_at(&self) -> &DateTime<Utc> {
        &self.updated_at
    }
}

// ---------------------------------------------------------------------------
// TemplateSummary — lightweight template listing
// ---------------------------------------------------------------------------

/// Lightweight summary returned by template list operations.
///
/// Contains enough information for a user to select a template without
/// loading the full template body.
///
/// # Contract (Frozen)
///
/// - `name` is always present and matches a filesystem-safe template name
/// - `step_count` is the number of steps in the template
/// - All fields are serializable for MCP transport
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TemplateSummary {
    /// Template name (filesystem-safe).
    name: String,

    /// Human-readable description.
    description: String,

    /// Template version.
    version: String,

    /// Tags for categorization.
    tags: Vec<String>,

    /// Number of steps in the template.
    step_count: usize,

    /// Last update timestamp (UTC).
    updated_at: DateTime<Utc>,
}

impl TemplateSummary {
    /// Create a new TemplateSummary.
    pub fn new(
        name: String,
        description: String,
        version: String,
        tags: Vec<String>,
        step_count: usize,
        updated_at: DateTime<Utc>,
    ) -> Self {
        Self {
            name,
            description,
            version,
            tags,
            step_count,
            updated_at,
        }
    }

    /// Template name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Template description.
    pub fn description(&self) -> &str {
        &self.description
    }

    /// Template version.
    pub fn version(&self) -> &str {
        &self.version
    }

    /// Tags for categorization.
    pub fn tags(&self) -> &[String] {
        &self.tags
    }

    /// Number of steps.
    pub fn step_count(&self) -> usize {
        self.step_count
    }

    /// Last update timestamp.
    pub fn updated_at(&self) -> &DateTime<Utc> {
        &self.updated_at
    }
}

// ---------------------------------------------------------------------------
// TemplateFilter — criteria for listing templates
// ---------------------------------------------------------------------------

/// Filter criteria for listing templates.
///
/// Supports filtering by tags, text search, and pagination via limit.
/// All filter fields are optional — empty filter returns all templates.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TemplateFilter {
    /// Optional tag filter — only templates matching ALL specified tags.
    #[serde(default)]
    tags: Option<Vec<String>>,

    /// Optional text search — matches against name and description.
    #[serde(default)]
    search: Option<String>,

    /// Maximum number of results to return (default: 50).
    #[serde(default = "default_limit")]
    limit: usize,
}

fn default_limit() -> usize {
    50
}

impl TemplateFilter {
    /// Create a new TemplateFilter with default limit.
    pub fn new(tags: Option<Vec<String>>, search: Option<String>, limit: Option<usize>) -> Self {
        Self {
            tags,
            search,
            limit: limit.unwrap_or(50),
        }
    }

    /// Optional tag filter.
    pub fn tags(&self) -> Option<&[String]> {
        self.tags.as_deref()
    }

    /// Optional text search.
    pub fn search(&self) -> Option<&str> {
        self.search.as_deref()
    }

    /// Maximum results.
    pub fn limit(&self) -> usize {
        self.limit
    }
}

impl Default for TemplateFilter {
    fn default() -> Self {
        Self {
            tags: None,
            search: None,
            limit: 50,
        }
    }
}

// ---------------------------------------------------------------------------
// StepDefinition — a single step in a template
// ---------------------------------------------------------------------------

/// A single step within a template: which tool to call, with what parameters,
/// and whether human approval is required.
///
/// # Contract (Frozen)
///
/// - Tool name must match a registered MCP tool
/// - Parameters are opaque JSON — validated by the target tool
/// - If `requires_approval` is true, execution pauses for human sign-off
/// - Optional timeout overrides the default step timeout
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StepDefinition {
    /// Step name (unique within the plan).
    name: String,

    /// MCP tool name to invoke.
    tool: String,

    /// Tool-specific parameters as a JSON object.
    parameters: serde_json::Value,

    /// Whether human approval is required before execution.
    requires_approval: bool,

    /// Human-readable description of the step's purpose.
    description: String,

    /// Optional timeout in seconds for this step.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    timeout_secs: Option<u64>,

    /// Whether to run scored evaluation on this step's output.
    #[serde(default)]
    evaluate_score: bool,
}

impl StepDefinition {
    /// Create a new StepDefinition.
    pub fn new(
        name: String,
        tool: String,
        parameters: serde_json::Value,
        requires_approval: bool,
        description: String,
        timeout_secs: Option<u64>,
    ) -> Self {
        Self {
            name,
            tool,
            parameters,
            requires_approval,
            description,
            timeout_secs,
            evaluate_score: false,
        }
    }

    /// Step name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// MCP tool name.
    pub fn tool(&self) -> &str {
        &self.tool
    }

    /// Tool-specific parameters.
    pub fn parameters(&self) -> &serde_json::Value {
        &self.parameters
    }

    /// Whether human approval is required.
    pub fn requires_approval(&self) -> bool {
        self.requires_approval
    }

    /// Step description.
    pub fn description(&self) -> &str {
        &self.description
    }

    /// Optional timeout in seconds.
    pub fn timeout_secs(&self) -> Option<u64> {
        self.timeout_secs
    }

    /// Whether to run scored evaluation on this step's output.
    pub fn evaluate_score(&self) -> bool {
        self.evaluate_score
    }

    /// Set whether to run scored evaluation.
    pub fn set_evaluate_score(&mut self, val: bool) {
        self.evaluate_score = val;
    }
}

// ---------------------------------------------------------------------------
// Constraints — enforcement boundaries for template execution
// ---------------------------------------------------------------------------

/// Enforcement constraints for template execution.
///
/// Optional boundaries that the plan must not exceed when instantiated
/// from this template.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Constraints {
    /// Maximum number of tool calls allowed.
    pub max_tool_calls: Option<u64>,

    /// Maximum total tokens allowed.
    pub max_tokens: Option<u64>,

    /// Maximum execution time in seconds.
    pub max_duration_secs: Option<u64>,

    /// List of disallowed tools.
    #[serde(default)]
    pub blocked_tools: Vec<String>,

    /// Additional constraint key-value pairs.
    #[serde(default)]
    pub extensions: HashMap<String, String>,
}

// ---------------------------------------------------------------------------
// GetTemplateInput — input for get template operation
// ---------------------------------------------------------------------------

/// Input for the `rigorix_get_template` tool call.
///
/// Specifies the template name and optional output format.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GetTemplateInput {
    /// Name of the template to retrieve.
    pub name: String,

    /// Optional output format ("json" or "toml"). Defaults to "json".
    pub format: Option<String>,
}

// ---------------------------------------------------------------------------
// CreateTemplateInput — input for create template operation
// ---------------------------------------------------------------------------

/// Input for the `rigorix_create_template` tool call.
///
/// Specifies the template name, plan, and overwrite behavior.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CreateTemplateInput {
    /// Name for the new template (filesystem-safe).
    pub name: String,

    /// The template plan to store.
    pub plan: PlanTemplate,

    /// Whether to overwrite an existing template with the same name.
    #[serde(default)]
    pub overwrite: bool,
}

// ---------------------------------------------------------------------------
// ValidateTemplateInput — input for validate template operation
// ---------------------------------------------------------------------------

/// Input for the `rigorix_validate_template` tool call.
///
/// Contains the template plan to validate.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ValidateTemplateInput {
    /// The template plan to validate against schema and enforcement policies.
    pub plan: PlanTemplate,
}
