//! HTTP API contracts for CLI Templates endpoints.
//!
//! @canonical .pi/architecture/modules/templates.md
//! Implements: Contract Freeze — HTTP endpoint contracts and error formats
//! Issue: issue-contract-freeze
//!
//! Defines endpoint paths, methods, request/response schemas, and error
//! response formats for CLI-to-engine template operations. These contracts
//! are framework-agnostic — they describe the API surface that any HTTP
//! server implementation must satisfy.
//!
//! The CLI templates module exposes two main operations:
//! - List all registered templates
//! - Show a specific template's definition
//!
//! These are thin wrappers over the engine's TemplateEngineService.
//!
//! # Contract (Frozen)
//! - All endpoints documented with method, path, request, and response types
//! - Error responses follow a unified format
//! - No framework-specific annotations (axum/actix/warp annotations added by implementation)

use serde::{Deserialize, Serialize};

use crate::templates::application::dto::{TemplateListOutput, TemplateShowOutput, TemplateSummary};

// ---------------------------------------------------------------------------
// API Base Path
// ---------------------------------------------------------------------------

/// All CLI template endpoints are served under this base path.
pub const API_BASE_PATH: &str = "/api/v1/cli/templates";

// ---------------------------------------------------------------------------
// Endpoint: GET /api/v1/cli/templates
// ---------------------------------------------------------------------------

/// GET /api/v1/cli/templates
///
/// List all registered templates with summary metadata.
///
/// **Response:** `200 OK` with `ListCliTemplatesResponse`
pub const LIST_CLI_TEMPLATES_PATH: &str = "/api/v1/cli/templates";
pub const LIST_CLI_TEMPLATES_METHOD: &str = "GET";

/// Response for GET /api/v1/cli/templates.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListCliTemplatesResponse {
    /// Summary of each registered template.
    pub templates: Vec<TemplateSummary>,
    /// Total count of registered templates.
    pub total: u32,
}

impl From<TemplateListOutput> for ListCliTemplatesResponse {
    fn from(output: TemplateListOutput) -> Self {
        Self {
            templates: output.templates,
            total: output.total,
        }
    }
}

// ---------------------------------------------------------------------------
// Endpoint: GET /api/v1/cli/templates/{id}
// ---------------------------------------------------------------------------

/// GET /api/v1/cli/templates/{id}
///
/// Get a specific template's TOML content.
///
/// **Response:** `200 OK` with `ShowCliTemplateResponse`
/// **Error:** `404 Not Found` with `CliApiErrorResponse`
pub const SHOW_CLI_TEMPLATE_PATH: &str = "/api/v1/cli/templates/{id}";
pub const SHOW_CLI_TEMPLATE_METHOD: &str = "GET";

/// Response for GET /api/v1/cli/templates/{id}.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShowCliTemplateResponse {
    /// The template TOML definition.
    pub content: String,
}

impl From<TemplateShowOutput> for ShowCliTemplateResponse {
    fn from(output: TemplateShowOutput) -> Self {
        Self {
            content: output.content,
        }
    }
}

// ---------------------------------------------------------------------------
// Unified Error Response Format
// ---------------------------------------------------------------------------

/// Standard error response for CLI Templates API endpoints.
///
/// All 4xx/5xx responses use this format.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CliApiErrorResponse {
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

/// Standardized error codes for CLI Templates API.
pub mod error_codes {
    /// Template not found.
    pub const NOT_FOUND: &str = "CLI_TEMPLATE_NOT_FOUND";
    /// Engine error during template operation.
    pub const ENGINE_ERROR: &str = "CLI_TEMPLATE_ENGINE_ERROR";
    /// Internal server error.
    pub const INTERNAL_ERROR: &str = "CLI_TEMPLATE_INTERNAL_ERROR";
}

/// HTTP status code mappings for CLI Templates errors.
pub mod status_codes {
    pub const NOT_FOUND: u16 = 404;
    pub const ENGINE_ERROR: u16 = 502;
    pub const INTERNAL_ERROR: u16 = 500;
}
