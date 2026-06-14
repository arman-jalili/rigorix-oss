//! HTTP API contracts for Template System endpoints.
//!
//! @canonical .pi/architecture/modules/template-system.md
//! Implements: Contract Freeze — HTTP endpoint contracts and error formats
//! Issue: #101
//!
//! Defines endpoint paths, methods, request/response schemas, and error
//! response formats. These contracts are framework-agnostic — they describe
//! the API surface that any HTTP server implementation must satisfy.
//!
//! # Contract (Frozen)
//! - All endpoints documented with method, path, request, and response types
//! - Error responses follow a unified format
//! - No framework-specific annotations (axum/actix/warp annotations added by implementation)

use serde::{Deserialize, Serialize};

use crate::templates::application::dto::{
    GenerateOutput, ParseOutput, ParseStrInput, RegisterOutput, TemplateSummary,
    ValidateTemplateOutput,
};

// ---------------------------------------------------------------------------
// API Base Path
// ---------------------------------------------------------------------------

/// All template system endpoints are served under this base path.
pub const API_BASE_PATH: &str = "/api/v1/templates";

// ---------------------------------------------------------------------------
// Endpoint: GET /api/v1/templates
// ---------------------------------------------------------------------------

/// GET /api/v1/templates
///
/// List all registered templates with summary metadata.
///
/// **Response:** `200 OK` with `ListTemplatesResponse`
pub const LIST_TEMPLATES_PATH: &str = "/api/v1/templates";
pub const LIST_TEMPLATES_METHOD: &str = "GET";

/// Response for GET /api/v1/templates.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListTemplatesResponse {
    /// Summary of each registered template.
    pub templates: Vec<TemplateSummary>,
    /// Total count of registered templates.
    pub total: usize,
}

// ---------------------------------------------------------------------------
// Endpoint: GET /api/v1/templates/{id}
// ---------------------------------------------------------------------------

/// GET /api/v1/templates/{id}
///
/// Get full details for a specific template.
///
/// **Response:** `200 OK` with `GetTemplateResponse`
/// **Error:** `404 Not Found` with `ApiErrorResponse`
pub const GET_TEMPLATE_PATH: &str = "/api/v1/templates/{id}";
pub const GET_TEMPLATE_METHOD: &str = "GET";

/// Response for GET /api/v1/templates/{id}.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetTemplateResponse {
    /// Full template definition.
    pub template: TemplateDetail,
}

/// Full template detail for API responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateDetail {
    /// Template metadata.
    pub id: String,
    pub name: String,
    pub description: String,
    pub version: String,
    /// Number of defined parameters.
    pub param_count: usize,
    /// Number of defined nodes.
    pub node_count: usize,
    pub tags: Vec<String>,
    pub category: Option<String>,
    pub author: Option<String>,
    /// Whether this is a built-in template.
    pub is_builtin: bool,
}

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/templates/parse
// ---------------------------------------------------------------------------

/// POST /api/v1/templates/parse
///
/// Parse a TOML template string and return the parsed result.
/// Does **not** register the template.
///
/// **Request:** `ParseTemplateRequest`
/// **Response:** `200 OK` with `ParseTemplateResponse`
/// **Error:** `400 Bad Request` with `ApiErrorResponse`
pub const PARSE_TEMPLATE_PATH: &str = "/api/v1/templates/parse";
pub const PARSE_TEMPLATE_METHOD: &str = "POST";

/// Request body for POST /api/v1/templates/parse.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParseTemplateRequest {
    /// TOML content to parse.
    pub toml_content: String,
    /// Optional source identifier.
    pub source: Option<String>,
    /// Whether to validate the parsed template.
    #[serde(default = "default_validate")]
    pub validate: bool,
}

fn default_validate() -> bool {
    true
}

impl From<ParseTemplateRequest> for ParseStrInput {
    fn from(req: ParseTemplateRequest) -> Self {
        Self {
            toml_content: req.toml_content,
            source: req.source,
            validate: req.validate,
        }
    }
}

/// Response for POST /api/v1/templates/parse.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParseTemplateResponse {
    pub success: bool,
    pub template_id: String,
    pub template_name: String,
    pub node_count: usize,
    pub param_count: usize,
    pub valid: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

impl From<ParseOutput> for ParseTemplateResponse {
    fn from(output: ParseOutput) -> Self {
        let template = output.template;
        Self {
            success: output.valid,
            template_id: template.id,
            template_name: template.name,
            node_count: template.nodes.len(),
            param_count: template.parameters.len(),
            valid: output.valid,
            errors: output.errors,
            warnings: output.warnings,
        }
    }
}

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/templates
// ---------------------------------------------------------------------------

/// POST /api/v1/templates
///
/// Register a parsed template in the engine's runtime registry.
///
/// **Request:** `RegisterTemplateRequest`
/// **Response:** `201 Created` with `RegisterTemplateResponse`
/// **Error:** `409 Conflict` if template ID already exists (use overwrite)
pub const REGISTER_TEMPLATE_PATH: &str = "/api/v1/templates";
pub const REGISTER_TEMPLATE_METHOD: &str = "POST";

/// Request body for POST /api/v1/templates.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterTemplateRequest {
    /// TOML content to parse and register.
    pub toml_content: String,
    /// Whether to overwrite if a template with the same ID exists.
    #[serde(default)]
    pub overwrite: bool,
}

/// Response for POST /api/v1/templates.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterTemplateResponse {
    pub success: bool,
    pub template_id: String,
    pub total_templates: usize,
    pub overwritten: bool,
}

impl From<RegisterOutput> for RegisterTemplateResponse {
    fn from(output: RegisterOutput) -> Self {
        Self {
            success: true,
            template_id: output.template_id,
            total_templates: output.total_templates,
            overwritten: output.overwritten,
        }
    }
}

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/templates/generate
// ---------------------------------------------------------------------------

/// POST /api/v1/templates/generate
///
/// Generate an executable graph from a registered template with parameters.
///
/// **Request:** `GenerateGraphRequest`
/// **Response:** `200 OK` with `GenerateGraphResponse`
/// **Error:** `404 Not Found` if template not registered
/// **Error:** `422 Unprocessable Entity` if parameters are invalid
pub const GENERATE_GRAPH_PATH: &str = "/api/v1/templates/generate";
pub const GENERATE_GRAPH_METHOD: &str = "POST";

/// Request body for POST /api/v1/templates/generate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateGraphRequest {
    /// Registered template ID.
    pub template_id: String,
    /// Parameter values for {{ param }} substitution.
    pub params: std::collections::HashMap<String, serde_json::Value>,
    /// Execution ID to associate with the generated graph.
    pub execution_id: uuid::Uuid,
}

/// Response for POST /api/v1/templates/generate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateGraphResponse {
    pub success: bool,
    pub template_id: String,
    pub node_count: usize,
    pub valid: bool,
    pub topological_order: Vec<String>,
    pub errors: Vec<String>,
    pub execution_id: uuid::Uuid,
}

impl From<GenerateOutput> for GenerateGraphResponse {
    fn from(output: GenerateOutput) -> Self {
        Self {
            success: output.valid,
            template_id: output.template_id,
            node_count: output.node_count,
            valid: output.valid,
            topological_order: output.topological_order,
            errors: output.errors,
            execution_id: output.execution_id,
        }
    }
}

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/templates/validate
// ---------------------------------------------------------------------------

/// POST /api/v1/templates/validate
///
/// Validate a template definition without registering it.
///
/// **Request:** `ValidateTemplateApiRequest`
/// **Response:** `200 OK` with `ValidateTemplateApiResponse`
pub const VALIDATE_TEMPLATE_PATH: &str = "/api/v1/templates/validate";
pub const VALIDATE_TEMPLATE_METHOD: &str = "POST";

/// Request body for POST /api/v1/templates/validate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidateTemplateApiRequest {
    /// TOML content to validate.
    pub toml_content: String,
    /// Whether to check for cycles.
    #[serde(default = "default_true")]
    pub check_cycles: bool,
    /// Whether to check parameter references.
    #[serde(default = "default_true")]
    pub check_param_references: bool,
}

fn default_true() -> bool {
    true
}

/// Response for POST /api/v1/templates/validate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidateTemplateApiResponse {
    pub valid: bool,
    pub errors: Vec<super::super::application::dto::ValidationError>,
    pub warnings: Vec<String>,
}

impl From<ValidateTemplateOutput> for ValidateTemplateApiResponse {
    fn from(output: ValidateTemplateOutput) -> Self {
        Self {
            valid: output.valid,
            errors: output.errors,
            warnings: output.warnings,
        }
    }
}

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/templates/load-builtins
// ---------------------------------------------------------------------------

/// POST /api/v1/templates/load-builtins
///
/// Load (or reload) built-in templates into the engine registry.
///
/// **Request:** `LoadBuiltinsApiRequest`
/// **Response:** `200 OK` with `LoadBuiltinsApiResponse`
pub const LOAD_BUILTINS_PATH: &str = "/api/v1/templates/load-builtins";
pub const LOAD_BUILTINS_METHOD: &str = "POST";

/// Request body for POST /api/v1/templates/load-builtins.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadBuiltinsApiRequest {
    /// Optional filter to load only specific categories.
    pub categories: Option<Vec<String>>,
    /// Whether to overwrite existing templates.
    #[serde(default)]
    pub overwrite: bool,
}

/// Response for POST /api/v1/templates/load-builtins.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadBuiltinsApiResponse {
    pub success: bool,
    pub loaded: Vec<String>,
    pub count: usize,
}

// ---------------------------------------------------------------------------
// Unified Error Response Format
// ---------------------------------------------------------------------------

/// Standard error response for all Template System API endpoints.
///
/// All 4xx/5xx responses use this format.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiErrorResponse {
    /// HTTP status code.
    pub status: u16,
    /// Machine-readable error code.
    pub code: String,
    /// Human-readable error message.
    pub message: String,
    /// Detailed error context (optional, may include field-level errors).
    pub details: Option<serde_json::Value>,
    /// Request ID for tracing (if available).
    pub request_id: Option<String>,
}

/// Standardized error codes for Template System API.
pub mod error_codes {
    /// Template not found.
    pub const NOT_FOUND: &str = "TEMPLATE_NOT_FOUND";
    /// Template parse error (invalid TOML).
    pub const PARSE_ERROR: &str = "TEMPLATE_PARSE_ERROR";
    /// Template validation failed.
    pub const VALIDATION_FAILED: &str = "TEMPLATE_VALIDATION_FAILED";
    /// Template already registered (conflict).
    pub const DUPLICATE_TEMPLATE: &str = "TEMPLATE_DUPLICATE";
    /// Missing required parameters.
    pub const MISSING_PARAMETER: &str = "TEMPLATE_MISSING_PARAMETER";
    /// Invalid parameter type or value.
    pub const INVALID_PARAMETER: &str = "TEMPLATE_INVALID_PARAMETER";
    /// Internal server error.
    pub const INTERNAL_ERROR: &str = "INTERNAL_ERROR";
}

/// HTTP status code mappings for Template System errors.
pub mod status_codes {
    pub const NOT_FOUND: u16 = 404;
    pub const PARSE_ERROR: u16 = 400;
    pub const VALIDATION_FAILED: u16 = 422;
    pub const DUPLICATE_TEMPLATE: u16 = 409;
    pub const MISSING_PARAMETER: u16 = 422;
    pub const INVALID_PARAMETER: u16 = 422;
    pub const INTERNAL_ERROR: u16 = 500;
}
