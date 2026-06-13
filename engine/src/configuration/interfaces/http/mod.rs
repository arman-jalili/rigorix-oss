//! HTTP API contracts for Configuration endpoints.
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

use crate::configuration::application::dto::{
    ConfigDto, LoadConfigInput, LoadConfigOutput, ValidateConfigInput, ValidateConfigOutput,
};

use super::super::application::dto::{LoadSecretInput, LoadSecretOutput};

// ---------------------------------------------------------------------------
// API Base Path
// ---------------------------------------------------------------------------

/// All configuration endpoints are served under this base path.
pub const API_BASE_PATH: &str = "/api/v1/config";

// ---------------------------------------------------------------------------
// Endpoint: GET /api/v1/config
// ---------------------------------------------------------------------------

/// GET /api/v1/config
///
/// Retrieve the current loaded configuration.
///
/// **Response:** `200 OK` with `GetConfigResponse`
pub const GET_CONFIG_PATH: &str = "/api/v1/config";
pub const GET_CONFIG_METHOD: &str = "GET";

/// Response for GET /api/v1/config.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetConfigResponse {
    /// The current configuration.
    pub config: ConfigDto,
    /// Sources that contributed to this configuration.
    pub sources: Vec<String>,
    /// Timestamp when config was loaded (ISO 8601).
    pub loaded_at: String,
}

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/config/load
// ---------------------------------------------------------------------------

/// POST /api/v1/config/load
///
/// Load (or reload) configuration from available sources.
///
/// **Request:** `LoadConfigRequest`
/// **Response:** `200 OK` with `LoadConfigResponse`
pub const LOAD_CONFIG_PATH: &str = "/api/v1/config/load";
pub const LOAD_CONFIG_METHOD: &str = "POST";

/// Request body for POST /api/v1/config/load.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadConfigRequest {
    /// Optional explicit config file path.
    pub config_path: Option<String>,
    /// CLI flag overrides (key = nested path with `__`, value = string).
    pub cli_overrides: Option<std::collections::HashMap<String, String>>,
    /// Whether to allow empty/missing config.
    pub allow_empty: Option<bool>,
}

impl From<LoadConfigRequest> for LoadConfigInput {
    fn from(req: LoadConfigRequest) -> Self {
        Self {
            config_path: req.config_path,
            env_prefix: None,
            cli_overrides: req.cli_overrides,
            allow_empty: req.allow_empty.unwrap_or(false),
        }
    }
}

/// Response body for POST /api/v1/config/load.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadConfigResponse {
    pub success: bool,
    pub config: ConfigDto,
    pub sources_used: Vec<String>,
    pub valid: bool,
}

impl From<LoadConfigOutput> for LoadConfigResponse {
    fn from(output: LoadConfigOutput) -> Self {
        Self {
            success: output.valid,
            config: output.config,
            sources_used: output.sources_used,
            valid: output.valid,
        }
    }
}

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/config/validate
// ---------------------------------------------------------------------------

/// POST /api/v1/config/validate
///
/// Validate a configuration against safety hard-caps without loading it.
///
/// **Request:** `ValidateConfigRequest`
/// **Response:** `200 OK` with `ValidateConfigResponse`
pub const VALIDATE_CONFIG_PATH: &str = "/api/v1/config/validate";
pub const VALIDATE_CONFIG_METHOD: &str = "POST";

/// Request body for POST /api/v1/config/validate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidateConfigRequest {
    /// The configuration to validate.
    pub config: ConfigDto,
    /// Optional safety caps override.
    pub safety_caps: Option<super::super::application::dto::SafetyCaps>,
}

impl From<ValidateConfigRequest> for ValidateConfigInput {
    fn from(req: ValidateConfigRequest) -> Self {
        Self {
            config: req.config,
            safety_caps: req.safety_caps,
        }
    }
}

/// Response body for POST /api/v1/config/validate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidateConfigResponse {
    pub valid: bool,
    pub errors: Vec<super::super::application::dto::ValidationError>,
    pub warnings: Vec<String>,
}

impl From<ValidateConfigOutput> for ValidateConfigResponse {
    fn from(output: ValidateConfigOutput) -> Self {
        Self {
            valid: output.valid,
            errors: output.errors,
            warnings: output.warnings,
        }
    }
}

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/config/secrets
// ---------------------------------------------------------------------------

/// POST /api/v1/config/secrets
///
/// Load a secret from an environment variable.
///
/// **Request:** `LoadSecretRequest`
/// **Response:** `200 OK` with `LoadSecretResponse`
pub const LOAD_SECRET_PATH: &str = "/api/v1/config/secrets";
pub const LOAD_SECRET_METHOD: &str = "POST";

/// Request body for POST /api/v1/config/secrets.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadSecretRequest {
    /// Environment variable name.
    pub env_var: String,
    /// Optional fallback value.
    pub fallback: Option<String>,
    /// Whether the secret is required.
    pub required: bool,
}

impl From<LoadSecretRequest> for LoadSecretInput {
    fn from(req: LoadSecretRequest) -> Self {
        Self {
            env_var: req.env_var,
            fallback: req.fallback,
            required: req.required,
        }
    }
}

/// Response body for POST /api/v1/config/secrets.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadSecretResponse {
    /// Whether the secret was found.
    pub found: bool,
    /// Source description (env var name or "fallback").
    pub source: String,
}

impl From<LoadSecretOutput> for LoadSecretResponse {
    fn from(output: LoadSecretOutput) -> Self {
        Self {
            found: !output.secret.is_empty(),
            source: output.source,
        }
    }
}

// ---------------------------------------------------------------------------
// Unified Error Response Format
// ---------------------------------------------------------------------------

/// Standard error response for all Configuration API endpoints.
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

/// Standardized error codes for Configuration API.
pub mod error_codes {
    /// Configuration file not found.
    pub const NOT_FOUND: &str = "CONFIG_NOT_FOUND";
    /// Configuration parse error (invalid TOML).
    pub const PARSE_ERROR: &str = "CONFIG_PARSE_ERROR";
    /// Configuration validation failed (value out of bounds).
    pub const VALIDATION_FAILED: &str = "CONFIG_VALIDATION_FAILED";
    /// Environment variable not found.
    pub const ENV_VAR_NOT_FOUND: &str = "ENV_VAR_NOT_FOUND";
    /// Internal server error.
    pub const INTERNAL_ERROR: &str = "INTERNAL_ERROR";
}

/// HTTP status code mappings for Configuration errors.
pub mod status_codes {
    pub const NOT_FOUND: u16 = 404;
    pub const PARSE_ERROR: u16 = 400;
    pub const VALIDATION_FAILED: u16 = 422;
    pub const ENV_VAR_NOT_FOUND: u16 = 404;
    pub const INTERNAL_ERROR: u16 = 500;
}
