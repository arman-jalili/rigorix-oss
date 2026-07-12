//! Data Transfer Objects for the Template Tools module.
//!
//! @canonical .pi/architecture/modules/template-tools.md#dto
//! Implements: Contract Freeze — all input/output DTO schemas
//!
//! DTOs define the input/output contracts for all service operations.
//! They carry documentation and validation metadata but no behavior.
//!
//! # Contract (Frozen)
//!
//! - Every service operation has a dedicated input and output DTO
//! - DTOs are serializable (JSON for API)
//! - Validation constraints are documented in field docs
//! - Fields use reasonable Rust types

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// List Templates DTOs
// ---------------------------------------------------------------------------

/// Output from `rigorix_list_templates` formatted as JSON for MCP response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListTemplatesOutput {
    /// List of template summaries matching the filter.
    pub templates: Vec<TemplateSummaryDto>,
}

/// Template summary DTO for list response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateSummaryDto {
    /// Template name.
    pub name: String,

    /// Human-readable description.
    pub description: String,

    /// Template version.
    pub version: String,

    /// Tags for categorization.
    pub tags: Vec<String>,

    /// Number of steps in the template.
    pub step_count: usize,

    /// Last update timestamp.
    pub updated_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Get Template DTOs
// ---------------------------------------------------------------------------

/// Output from `rigorix_get_template` formatted as JSON for MCP response.
///
/// The `content` field contains the template in the requested format
/// (JSON object or TOML string).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetTemplateOutput {
    /// Template name.
    pub name: String,

    /// Template description.
    pub description: String,

    /// Template version.
    pub version: String,

    /// Tags for categorization.
    pub tags: Vec<String>,

    /// Ordered list of steps.
    pub steps: Vec<StepDefinitionDto>,

    /// Optional enforcement constraints.
    pub constraints: Option<ConstraintsDto>,

    /// Extensible metadata.
    pub metadata: std::collections::HashMap<String, String>,

    /// Creation timestamp.
    pub created_at: DateTime<Utc>,

    /// Last update timestamp.
    pub updated_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Create Template DTOs
// ---------------------------------------------------------------------------

/// Output from `rigorix_create_template` formatted as JSON for MCP response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTemplateOutput {
    /// Name of the created template.
    pub name: String,

    /// Path to the template file on disk.
    pub path: String,

    /// Creation status.
    pub status: String,
}

// ---------------------------------------------------------------------------
// Validate Template DTOs
// ---------------------------------------------------------------------------

/// Output from `rigorix_validate_template` formatted as JSON for MCP response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidateTemplateOutput {
    /// Whether the template is valid.
    pub valid: bool,

    /// Warning messages (non-blocking issues).
    #[serde(default)]
    pub warnings: Vec<String>,

    /// Error messages (blocking issues).
    #[serde(default)]
    pub errors: Vec<String>,

    /// Optional estimated cost of execution from enforcement policies.
    #[serde(default)]
    pub estimated_cost: Option<CostEstimateDto>,
}

/// Estimated cost DTO for validation responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostEstimateDto {
    /// Estimated token count.
    pub estimated_tokens: u64,

    /// Estimated tool calls.
    pub estimated_tool_calls: u64,

    /// Optional monetary estimate (in micro-units).
    pub estimated_cost_micro: Option<u64>,
}

// ---------------------------------------------------------------------------
// Shared DTOs
// ---------------------------------------------------------------------------

/// Step definition DTO used in template output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepDefinitionDto {
    /// Step name.
    pub name: String,

    /// MCP tool name to invoke.
    pub tool: String,

    /// Tool-specific parameters.
    pub parameters: serde_json::Value,

    /// Whether human approval is required.
    pub requires_approval: bool,

    /// Step description.
    pub description: String,

    /// Optional timeout in seconds.
    #[serde(default)]
    pub timeout_secs: Option<u64>,
}

/// Constraints DTO for template output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConstraintsDto {
    /// Maximum tool calls.
    pub max_tool_calls: Option<u64>,

    /// Maximum tokens.
    pub max_tokens: Option<u64>,

    /// Maximum duration in seconds.
    pub max_duration_secs: Option<u64>,

    /// Disallowed tools.
    pub blocked_tools: Vec<String>,

    /// Additional constraints.
    pub extensions: std::collections::HashMap<String, String>,
}
