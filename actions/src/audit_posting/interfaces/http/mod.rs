//! HTTP API contracts for Audit Posting endpoints.
//!
//! @canonical actions/.pi/architecture/modules/audit-posting.md
//! Implements: Contract Freeze — HTTP endpoint contracts and error formats
//! Issue: issue-contract-freeze
//!
//! Defines endpoint paths, methods, request/response schemas, and error
//! response formats. These contracts are framework-agnostic — they describe
//! the API surface that any HTTP server implementation must satisfy.
//!
//! Note: In production, audit posting runs inside a GitHub Action, not an
//! HTTP server. These contracts exist for:
//! - Local development & debugging endpoints
//! - Runtime introspection (health checks, status)
//! - Testing via HTTP mocks
//!
//! # Contract (Frozen)
//! - All endpoints documented with method, path, request, and response types
//! - Error responses follow a unified format
//! - No framework-specific annotations (axum/actix/warp annotations added by implementation)

use serde::{Deserialize, Serialize};

use crate::audit_posting::domain::SignedAuditRecord;

use crate::audit_posting::application::dto::PostResultDto;

// ---------------------------------------------------------------------------
// API Base Path
// ---------------------------------------------------------------------------

/// All audit posting endpoints are served under this base path.
pub const API_BASE_PATH: &str = "/api/v1/audit-posting";

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/audit-posting/records
// ---------------------------------------------------------------------------

/// POST /api/v1/audit-posting/records
///
/// Create a signed audit record and optionally post it.
///
/// **Request:** `CreateRecordRequest`
/// **Response:** `201 Created` with `CreateRecordResponse`
pub const CREATE_RECORD_PATH: &str = "/api/v1/audit-posting/records";
pub const CREATE_RECORD_METHOD: &str = "POST";

/// Request body for POST /api/v1/audit-posting/records.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateRecordRequest {
    /// Globally unique execution identifier.
    pub execution_id: uuid::Uuid,

    /// The GitHub Actions workflow run ID, if applicable.
    pub run_id: Option<u64>,

    /// The GitHub Actions workflow name.
    pub workflow_name: Option<String>,

    /// The repository owner/name.
    pub repository: String,

    /// The git ref (branch or tag).
    pub git_ref: Option<String>,

    /// The commit SHA.
    pub commit_sha: Option<String>,

    /// The execution mode.
    pub mode: String,

    /// Human-readable summary.
    pub summary: String,

    /// Optional actor identity.
    pub actor: Option<String>,

    /// Optional key-value metadata.
    pub metadata: Option<std::collections::HashMap<String, String>>,

    /// Whether to sign the record with HMAC.
    #[serde(default)]
    pub sign: bool,

    /// Whether to post immediately after creating.
    #[serde(default)]
    pub post_immediately: bool,
}

impl From<CreateRecordRequest> for crate::audit_posting::application::dto::CreateRecordInput {
    fn from(req: CreateRecordRequest) -> Self {
        Self {
            execution_id: req.execution_id,
            run_id: req.run_id,
            workflow_name: req.workflow_name,
            repository: req.repository,
            git_ref: req.git_ref,
            commit_sha: req.commit_sha,
            mode: req.mode,
            summary: req.summary,
            actor: req.actor,
            metadata: req.metadata,
            sign: req.sign,
            post_immediately: req.post_immediately,
        }
    }
}

/// Response body for POST /api/v1/audit-posting/records.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateRecordResponse {
    pub success: bool,
    pub record: SignedAuditRecord,
    pub signed: bool,
    pub posted: bool,
    pub post_result: Option<PostResultDto>,
}

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/audit-posting/records/post
// ---------------------------------------------------------------------------

/// POST /api/v1/audit-posting/records/post
///
/// Post an existing signed audit record to the backend.
///
/// **Request:** `PostRecordRequest`
/// **Response:** `200 OK` with `PostRecordResponse`
pub const POST_RECORD_PATH: &str = "/api/v1/audit-posting/records/post";
pub const POST_RECORD_METHOD: &str = "POST";

/// Request body for POST /api/v1/audit-posting/records/post.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostRecordRequest {
    pub record: SignedAuditRecord,
    pub backend_url: Option<String>,
    pub timeout_secs: Option<u64>,
}

/// Response body for POST /api/v1/audit-posting/records/post.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostRecordResponse {
    pub success: bool,
    pub http_status: Option<u16>,
    pub duration_ms: u64,
    pub backend_url: String,
}

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/audit-posting/records/sign
// ---------------------------------------------------------------------------

/// POST /api/v1/audit-posting/records/sign
///
/// Sign an audit record with HMAC-SHA256.
///
/// **Request:** `SignRecordRequest`
/// **Response:** `200 OK` with `SignRecordResponse`
pub const SIGN_RECORD_PATH: &str = "/api/v1/audit-posting/records/sign";
pub const SIGN_RECORD_METHOD: &str = "POST";

/// Request body for POST /api/v1/audit-posting/records/sign.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignRecordRequest {
    pub record: SignedAuditRecord,
    pub key_id: Option<String>,
}

/// Response body for POST /api/v1/audit-posting/records/sign.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignRecordResponse {
    pub success: bool,
    pub record: SignedAuditRecord,
    pub signature: String,
    pub key_id: String,
}

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/audit-posting/records/verify
// ---------------------------------------------------------------------------

/// POST /api/v1/audit-posting/records/verify
///
/// Verify an audit record's HMAC signature.
///
/// **Request:** `VerifyRecordRequest`
/// **Response:** `200 OK` with `VerifyRecordResponse`
pub const VERIFY_RECORD_PATH: &str = "/api/v1/audit-posting/records/verify";
pub const VERIFY_RECORD_METHOD: &str = "POST";

/// Request body for POST /api/v1/audit-posting/records/verify.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifyRecordRequest {
    pub record: SignedAuditRecord,
    pub key_id: Option<String>,
}

/// Response body for POST /api/v1/audit-posting/records/verify.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifyRecordResponse {
    pub valid: bool,
    pub key_id: Option<String>,
    pub detail: Option<String>,
}

// ---------------------------------------------------------------------------
// Endpoint: GET /api/v1/audit-posting/records/{execution_id}
// ---------------------------------------------------------------------------

/// GET /api/v1/audit-posting/records/{execution_id}
///
/// Load an audit record by execution ID.
///
/// **Response:** `200 OK` with `LoadRecordResponse`
/// **Response:** `404 Not Found` with `ApiErrorResponse`
pub const LOAD_RECORD_PATH: &str = "/api/v1/audit-posting/records/:execution_id";
pub const LOAD_RECORD_METHOD: &str = "GET";

/// Response body for GET /api/v1/audit-posting/records/{execution_id}.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadRecordResponse {
    pub found: bool,
    pub record: Option<SignedAuditRecord>,
    pub signature_valid: Option<bool>,
}

// ---------------------------------------------------------------------------
// Endpoint: GET /api/v1/audit-posting/records
// ---------------------------------------------------------------------------

/// GET /api/v1/audit-posting/records
///
/// List audit records, optionally filtered by date range.
///
/// **Query params:** `since`, `until`, `limit`
/// **Response:** `200 OK` with `ListRecordsResponse`
pub const LIST_RECORDS_PATH: &str = "/api/v1/audit-posting/records";
pub const LIST_RECORDS_METHOD: &str = "GET";

/// Response body for GET /api/v1/audit-posting/records.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListRecordsResponse {
    pub records: Vec<SignedAuditRecord>,
    pub total_count: u64,
    pub since: Option<chrono::DateTime<chrono::Utc>>,
    pub until: Option<chrono::DateTime<chrono::Utc>>,
    pub limit: Option<u32>,
}

// ---------------------------------------------------------------------------
// Endpoint: GET /api/v1/audit-posting/status
// ---------------------------------------------------------------------------

/// GET /api/v1/audit-posting/status
///
/// Get the current audit posting system status.
///
/// **Response:** `200 OK` with `PostingStatusResponse`
pub const POSTING_STATUS_PATH: &str = "/api/v1/audit-posting/status";
pub const POSTING_STATUS_METHOD: &str = "GET";

/// Response body for GET /api/v1/audit-posting/status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostingStatusResponse {
    pub pending_count: u32,
    pub backend_available: bool,
    pub total_posted: u64,
    pub total_failed: u64,
    pub on_disk_count: Option<u64>,
}

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/audit-posting/retry
// ---------------------------------------------------------------------------

/// POST /api/v1/audit-posting/retry
///
/// Retry all pending records in the delivery queue.
///
/// **Response:** `200 OK` with `RetryPendingResponse`
pub const RETRY_PENDING_PATH: &str = "/api/v1/audit-posting/retry";
pub const RETRY_PENDING_METHOD: &str = "POST";

/// Response body for POST /api/v1/audit-posting/retry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryPendingResponse {
    pub delivered: u32,
    pub still_pending: u32,
    pub dropped: u32,
}

// ---------------------------------------------------------------------------
// Endpoint: DELETE /api/v1/audit-posting/records/{execution_id}
// ---------------------------------------------------------------------------

/// DELETE /api/v1/audit-posting/records/{execution_id}
///
/// Delete an audit record by execution ID.
///
/// **Response:** `204 No Content`
pub const DELETE_RECORD_PATH: &str = "/api/v1/audit-posting/records/:execution_id";
pub const DELETE_RECORD_METHOD: &str = "DELETE";

// ---------------------------------------------------------------------------
// Endpoint: GET /api/v1/audit-posting/health
// ---------------------------------------------------------------------------

/// GET /api/v1/audit-posting/health
///
/// Check the audit backend health.
///
/// **Response:** `200 OK` with `HealthCheckResponse`
pub const HEALTH_CHECK_PATH: &str = "/api/v1/audit-posting/health";
pub const HEALTH_CHECK_METHOD: &str = "GET";

/// Response body for GET /api/v1/audit-posting/health.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckResponse {
    pub healthy: bool,
    pub backend_type: String,
    pub storage_path: Option<String>,
}

// ---------------------------------------------------------------------------
// Unified Error Response Format
// ---------------------------------------------------------------------------

/// Standard error response for all Audit Posting API endpoints.
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

/// Standardized error codes for Audit Posting API.
pub mod error_codes {
    /// Signing failed.
    pub const SIGNING_FAILED: &str = "AUDIT_SIGNING_FAILED";
    /// Post failed (backend error).
    pub const POST_FAILED: &str = "AUDIT_POST_FAILED";
    /// Backend unavailable.
    pub const BACKEND_UNAVAILABLE: &str = "AUDIT_BACKEND_UNAVAILABLE";
    /// Serialization failed.
    pub const SERIALIZATION_FAILED: &str = "AUDIT_SERIALIZATION_FAILED";
    /// Signature mismatch.
    pub const SIGNATURE_MISMATCH: &str = "AUDIT_SIGNATURE_MISMATCH";
    /// Queue is full.
    pub const QUEUE_FULL: &str = "AUDIT_QUEUE_FULL";
    /// Not configured.
    pub const NOT_CONFIGURED: &str = "AUDIT_NOT_CONFIGURED";
    /// Record not found.
    pub const RECORD_NOT_FOUND: &str = "AUDIT_RECORD_NOT_FOUND";
    /// Filesystem error.
    pub const FILESYSTEM_ERROR: &str = "AUDIT_FILESYSTEM_ERROR";
    /// Key not available.
    pub const KEY_NOT_AVAILABLE: &str = "AUDIT_KEY_NOT_AVAILABLE";
    /// Internal server error.
    pub const INTERNAL_ERROR: &str = "AUDIT_INTERNAL_ERROR";
}

/// HTTP status code mappings for Audit Posting errors.
pub mod status_codes {
    pub const SIGNING_FAILED: u16 = 500;
    pub const POST_FAILED: u16 = 502;
    pub const BACKEND_UNAVAILABLE: u16 = 503;
    pub const SERIALIZATION_FAILED: u16 = 500;
    pub const SIGNATURE_MISMATCH: u16 = 400;
    pub const QUEUE_FULL: u16 = 429;
    pub const NOT_CONFIGURED: u16 = 503;
    pub const RECORD_NOT_FOUND: u16 = 404;
    pub const FILESYSTEM_ERROR: u16 = 500;
    pub const KEY_NOT_AVAILABLE: u16 = 503;
    pub const INTERNAL_ERROR: u16 = 500;
}
