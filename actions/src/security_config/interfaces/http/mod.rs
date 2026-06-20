//! HTTP API contracts for Security Configuration endpoints.
//!
//! @canonical actions/.pi/architecture/modules/security-config.md
//! Implements: Contract Freeze — HTTP endpoint contracts and error formats
//! Issue: issue-contract-freeze
//!
//! Defines endpoint paths, methods, request/response schemas, and error
//! response formats. These contracts are framework-agnostic — they describe
//! the API surface that any HTTP server implementation must satisfy.
//!
//! Note: In production, security checks run before the action pipeline.
//! These contracts exist for:
//! - Local development & debugging endpoints
//! - Runtime introspection (health checks, status)
//! - Testing via HTTP mocks
//!
//! # Contract (Frozen)
//! - All endpoints documented with method, path, request, and response types
//! - Error responses follow a unified format
//! - No framework-specific annotations (axum/actix/warp annotations added by implementation)

use serde::{Deserialize, Serialize};

use crate::security_config::domain::{HmacKey, SecurityContext, SecurityLevel, SecurityPolicy};

use super::super::application::dto::{
    DetectForkOutput, HmacSignOutput, HmacVerifyOutput, MaskSecretsOutput, ValidateSecurityOutput,
    ValidateTokenOutput, ValidateUrlOutput,
};

// ---------------------------------------------------------------------------
// API Base Path
// ---------------------------------------------------------------------------

/// All security config endpoints are served under this base path.
pub const API_BASE_PATH: &str = "/api/v1/security";

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/security/validate
// ---------------------------------------------------------------------------

/// POST /api/v1/security/validate
///
/// Run all pre-flight security checks.
///
/// **Request:** `ValidateRequest`
/// **Response:** `200 OK` with `ValidateResponse`
pub const VALIDATE_PATH: &str = "/api/v1/security/validate";
pub const VALIDATE_METHOD: &str = "POST";

/// Request body for POST /api/v1/security/validate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidateRequest {
    /// The GitHub token to validate.
    pub github_token: String,
    /// Optional API key to mask.
    pub api_key: Option<String>,
    /// Path to the policy file.
    pub policy_path: String,
    /// Backend audit URL to validate.
    pub backend_url: Option<String>,
    /// Action mode to check permissions for.
    pub mode: String,
}

/// Response body for POST /api/v1/security/validate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidateResponse {
    pub success: bool,
    pub context: SecurityContext,
    pub warnings: Vec<String>,
    pub duration_ms: u64,
}

// ---------------------------------------------------------------------------
// Endpoint: GET /api/v1/security/context
// ---------------------------------------------------------------------------

/// GET /api/v1/security/context
///
/// Get the current security context.
///
/// **Response:** `200 OK` with `GetContextResponse`
pub const GET_CONTEXT_PATH: &str = "/api/v1/security/context";
pub const GET_CONTEXT_METHOD: &str = "GET";

/// Response for GET /api/v1/security/context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetContextResponse {
    pub context: SecurityContext,
    pub level: SecurityLevel,
    pub is_allowed: bool,
}

// ---------------------------------------------------------------------------
// Endpoint: GET /api/v1/security/fork
// ---------------------------------------------------------------------------

/// GET /api/v1/security/fork
///
/// Detect if this is a fork PR.
///
/// **Response:** `200 OK` with `ForkDetectionResponse`
pub const FORK_DETECTION_PATH: &str = "/api/v1/security/fork";
pub const FORK_DETECTION_METHOD: &str = "GET";

/// Response for GET /api/v1/security/fork.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForkDetectionResponse {
    pub is_fork: bool,
    pub head_repo: Option<String>,
    pub base_repo: Option<String>,
    pub fork_owner: Option<String>,
}

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/security/mask
// ---------------------------------------------------------------------------

/// POST /api/v1/security/mask
///
/// Mask secrets from workflow logs.
///
/// **Request:** `MaskRequest`
/// **Response:** `200 OK` with `MaskResponse`
pub const MASK_PATH: &str = "/api/v1/security/mask";
pub const MASK_METHOD: &str = "POST";

/// Request body for POST /api/v1/security/mask.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaskRequest {
    pub secrets: Vec<String>,
}

/// Response body for POST /api/v1/security/mask.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaskResponse {
    pub masked_count: u32,
    pub masked_hints: Vec<String>,
}

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/security/token
// ---------------------------------------------------------------------------

/// POST /api/v1/security/token
///
/// Validate the GitHub token and check permissions.
///
/// **Request:** `TokenValidateRequest`
/// **Response:** `200 OK` with `TokenValidateResponse`
pub const TOKEN_VALIDATE_PATH: &str = "/api/v1/security/token";
pub const TOKEN_VALIDATE_METHOD: &str = "POST";

/// Request body for POST /api/v1/security/token.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenValidateRequest {
    pub token: String,
    pub mode: String,
}

/// Response body for POST /api/v1/security/token.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenValidateResponse {
    pub valid: bool,
    pub has_required_permissions: bool,
    pub available_scopes: Vec<String>,
    pub missing_scopes: Vec<String>,
}

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/security/url
// ---------------------------------------------------------------------------

/// POST /api/v1/security/url
///
/// Validate a URL against the allowlist.
///
/// **Request:** `UrlValidateRequest`
/// **Response:** `200 OK` with `UrlValidateResponse`
pub const URL_VALIDATE_PATH: &str = "/api/v1/security/url";
pub const URL_VALIDATE_METHOD: &str = "POST";

/// Request body for POST /api/v1/security/url.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UrlValidateRequest {
    pub url: String,
}

/// Response body for POST /api/v1/security/url.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UrlValidateResponse {
    pub allowed: bool,
    pub host: String,
    pub checked_against: Vec<String>,
}

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/security/hmac/sign
// ---------------------------------------------------------------------------

/// POST /api/v1/security/hmac/sign
///
/// Sign a payload with HMAC-SHA256.
///
/// **Request:** `HmacSignRequest`
/// **Response:** `200 OK` with `HmacSignResponse`
pub const HMAC_SIGN_PATH: &str = "/api/v1/security/hmac/sign";
pub const HMAC_SIGN_METHOD: &str = "POST";

/// Request body for POST /api/v1/security/hmac/sign.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HmacSignRequest {
    pub payload: String,
}

/// Response body for POST /api/v1/security/hmac/sign.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HmacSignResponse {
    pub signature: String,
    pub key_id: String,
}

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/security/hmac/verify
// ---------------------------------------------------------------------------

/// POST /api/v1/security/hmac/verify
///
/// Verify an HMAC-SHA256 signature.
///
/// **Request:** `HmacVerifyRequest`
/// **Response:** `200 OK` with `HmacVerifyResponse`
pub const HMAC_VERIFY_PATH: &str = "/api/v1/security/hmac/verify";
pub const HMAC_VERIFY_METHOD: &str = "POST";

/// Request body for POST /api/v1/security/hmac/verify.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HmacVerifyRequest {
    pub payload: String,
    pub signature: String,
}

/// Response body for POST /api/v1/security/hmac/verify.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HmacVerifyResponse {
    pub valid: bool,
    pub key_id: String,
}

// ---------------------------------------------------------------------------
// Unified Error Response Format
// ---------------------------------------------------------------------------

/// Standard error response for all Security Configuration API endpoints.
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

/// Standardized error codes for Security Configuration API.
pub mod error_codes {
    /// Fork PR detected.
    pub const FORK_DETECTED: &str = "FORK_DETECTED";
    /// Token validation failed.
    pub const TOKEN_VALIDATION_FAILED: &str = "TOKEN_VALIDATION_FAILED";
    /// Token has insufficient permissions.
    pub const TOKEN_INSUFFICIENT: &str = "TOKEN_INSUFFICIENT";
    /// URL blocked by allowlist.
    pub const URL_BLOCKED: &str = "URL_BLOCKED";
    /// Invalid URL format.
    pub const INVALID_URL: &str = "INVALID_URL";
    /// HMAC signature verification failed.
    pub const HMAC_VERIFICATION_FAILED: &str = "HMAC_VERIFICATION_FAILED";
    /// HMAC key not available.
    pub const HMAC_KEY_MISSING: &str = "HMAC_KEY_MISSING";
    /// Policy file not found.
    pub const POLICY_NOT_FOUND: &str = "POLICY_NOT_FOUND";
    /// Policy parse error.
    pub const POLICY_PARSE_ERROR: &str = "POLICY_PARSE_ERROR";
    /// Internal server error.
    pub const INTERNAL_ERROR: &str = "INTERNAL_ERROR";
}

/// HTTP status code mappings for Security Configuration errors.
pub mod status_codes {
    pub const FORK_DETECTED: u16 = 403;
    pub const TOKEN_VALIDATION_FAILED: u16 = 401;
    pub const TOKEN_INSUFFICIENT: u16 = 403;
    pub const URL_BLOCKED: u16 = 403;
    pub const INVALID_URL: u16 = 400;
    pub const HMAC_VERIFICATION_FAILED: u16 = 401;
    pub const HMAC_KEY_MISSING: u16 = 500;
    pub const POLICY_NOT_FOUND: u16 = 404;
    pub const POLICY_PARSE_ERROR: u16 = 400;
    pub const INTERNAL_ERROR: u16 = 500;
}
