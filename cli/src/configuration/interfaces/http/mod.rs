//! HTTP API contracts for CLI Configuration endpoints.
//!
//! @canonical .pi/architecture/modules/configuration.md
//! Implements: Contract Freeze — HTTP endpoint contracts and error formats
//! Issue: issue-contract-freeze
//!
//! Defines endpoint paths, methods, request/response schemas, and error
//! response formats for CLI configuration operations. These contracts
//! are framework-agnostic — they describe the API surface that any HTTP
//! server implementation must satisfy.
//!
//! The CLI configuration module exposes operations for:
//! - Loading configuration from files/env/flags
//! - Querying current config values
//! - Validating configuration completeness
//!
//! # Contract (Frozen)
//! - All endpoints documented with method, path, request, and response types
//! - Error responses follow a unified format
//! - No framework-specific annotations (axum/actix/warp annotations added by implementation)

use serde::{Deserialize, Serialize};

use crate::configuration::application::dto::{
    ConfigSource, LoadConfigOutput, ValidateConfigOutput,
};

// ---------------------------------------------------------------------------
// API Base Path
// ---------------------------------------------------------------------------

/// All CLI configuration endpoints are served under this base path.
pub const API_BASE_PATH: &str = "/api/v1/cli/config";

// ---------------------------------------------------------------------------
// Endpoint: GET /api/v1/cli/config
// ---------------------------------------------------------------------------

/// GET /api/v1/cli/config
///
/// Get the current CLI configuration.
///
/// **Response:** `200 OK` with `ConfigResponse`
pub const GET_CONFIG_PATH: &str = "/api/v1/cli/config";
pub const GET_CONFIG_METHOD: &str = "GET";

/// Response for GET /api/v1/cli/config.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigResponse {
    /// The output format.
    pub output_format: String,
    /// Whether TUI is enabled.
    pub tui_enabled: bool,
    /// The color mode.
    pub color: String,
    /// The log level.
    pub log_level: String,
    /// Whether an API key is configured.
    pub api_key_configured: bool,
    /// The primary source of configuration.
    pub primary_source: String,
}

impl From<LoadConfigOutput> for ConfigResponse {
    fn from(output: LoadConfigOutput) -> Self {
        Self {
            output_format: String::new(),
            tui_enabled: false,
            color: String::new(),
            log_level: String::new(),
            api_key_configured: output.api_key_configured,
            primary_source: format!("{:?}", output.primary_source),
        }
    }
}

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/cli/config/validate
// ---------------------------------------------------------------------------

/// POST /api/v1/cli/config/validate
///
/// Validate the current configuration.
///
/// **Request:** `ValidateConfigApiRequest`
/// **Response:** `200 OK` with `ValidateConfigApiResponse`
pub const VALIDATE_CONFIG_PATH: &str = "/api/v1/cli/config/validate";
pub const VALIDATE_CONFIG_METHOD: &str = "POST";

/// Request body for POST /api/v1/cli/config/validate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidateConfigApiRequest {
    /// Whether to check for the API key.
    #[serde(default = "default_true")]
    pub check_api_key: bool,
}

fn default_true() -> bool {
    true
}

/// Response for POST /api/v1/cli/config/validate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidateConfigApiResponse {
    pub valid: bool,
    pub api_key_configured: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

impl From<ValidateConfigOutput> for ValidateConfigApiResponse {
    fn from(output: ValidateConfigOutput) -> Self {
        Self {
            valid: output.valid,
            api_key_configured: output.api_key_configured,
            errors: output.errors,
            warnings: output.warnings,
        }
    }
}

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/cli/config/reload
// ---------------------------------------------------------------------------

/// POST /api/v1/cli/config/reload
///
/// Reload configuration from the config file.
///
/// **Response:** `200 OK` with `ReloadConfigApiResponse`
/// **Error:** `500 Internal Server Error` with `CliApiErrorResponse`
pub const RELOAD_CONFIG_PATH: &str = "/api/v1/cli/config/reload";
pub const RELOAD_CONFIG_METHOD: &str = "POST";

/// Response for POST /api/v1/cli/config/reload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReloadConfigApiResponse {
    pub success: bool,
    pub config_path: Option<String>,
    pub sources_used: Vec<ConfigSource>,
}

// ---------------------------------------------------------------------------
// Unified Error Response Format
// ---------------------------------------------------------------------------

/// Standard error response for CLI Configuration API endpoints.
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
    /// Detailed error context (optional).
    pub details: Option<serde_json::Value>,
    /// Request ID for tracing.
    pub request_id: Option<String>,
}

/// Standardized error codes for CLI Configuration API.
pub mod error_codes {
    /// Configuration file not found.
    pub const NOT_FOUND: &str = "CONFIG_NOT_FOUND";
    /// Configuration parse error.
    pub const PARSE_ERROR: &str = "CONFIG_PARSE_ERROR";
    /// Missing required configuration value.
    pub const MISSING_VALUE: &str = "CONFIG_MISSING_VALUE";
    /// Internal server error.
    pub const INTERNAL_ERROR: &str = "CONFIG_INTERNAL_ERROR";
}

/// HTTP status code mappings for CLI Configuration errors.
pub mod status_codes {
    pub const NOT_FOUND: u16 = 404;
    pub const PARSE_ERROR: u16 = 400;
    pub const MISSING_VALUE: u16 = 422;
    pub const INTERNAL_ERROR: u16 = 500;
}
