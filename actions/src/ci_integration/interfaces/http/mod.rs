//! HTTP API contracts for CI Integration endpoints.
//!
//! @canonical actions/.pi/architecture/modules/ci-integration.md
//! Implements: Contract Freeze — HTTP endpoint contracts and error formats
//! Issue: issue-contract-freeze
//!
//! Defines endpoint paths, methods, request/response schemas, and error
//! response formats. These contracts are framework-agnostic — they describe
//! the API surface that any HTTP server implementation must satisfy.
//!
//! Note: In production, the action runs as a GitHub Action, not an HTTP server.
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

use crate::ci_integration::domain::{
    ExecutionSummary, GitHubStatus, PrComment, StatusCheckState,
};

// ---------------------------------------------------------------------------
// API Base Path
// ---------------------------------------------------------------------------

/// All CI integration endpoints are served under this base path.
pub const API_BASE_PATH: &str = "/api/v1/ci-integration";

// ---------------------------------------------------------------------------
// Unified Error Response
// ---------------------------------------------------------------------------

/// Standard error response format for all CI integration endpoints.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiErrorResponse {
    /// A machine-readable error code.
    pub error: String,
    /// Human-readable error message.
    pub message: String,
    /// Additional error details (optional).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

impl ApiErrorResponse {
    /// Create a new error response.
    pub fn new(error: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            error: error.into(),
            message: message.into(),
            details: None,
        }
    }

    /// Attach additional details to the error response.
    pub fn with_details(mut self, details: serde_json::Value) -> Self {
        self.details = Some(details);
        self
    }
}

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/ci-integration/status
// ---------------------------------------------------------------------------

/// POST /api/v1/ci-integration/status
///
/// Create or update a commit status check.
///
/// **Request Body:** `CreateStatusRequest`
/// **Response:** `201 Created` with `StatusResponse`
pub const CREATE_STATUS_PATH: &str = "/api/v1/ci-integration/status";
pub const CREATE_STATUS_METHOD: &str = "POST";

/// Request body for creating or updating a commit status check.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateStatusRequest {
    /// Repository owner (e.g., "rigorix").
    pub owner: String,
    /// Repository name (e.g., "rigorix-oss").
    pub repo: String,
    /// The full commit SHA.
    pub commit_sha: String,
    /// The status check details.
    pub status: GitHubStatus,
}

/// Response from creating or updating a commit status check.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusResponse {
    /// The resulting status check state.
    pub state: StatusCheckState,
    /// The context string used.
    pub context: String,
    /// The API-assigned status check ID if available.
    pub status_id: Option<u64>,
}

// ---------------------------------------------------------------------------
// Endpoint: GET /api/v1/ci-integration/status/{owner}/{repo}/{sha}
// ---------------------------------------------------------------------------

/// GET /api/v1/ci-integration/status/{owner}/{repo}/{sha}
///
/// Get all status checks for a commit.
///
/// **Response:** `200 OK` with `ListStatusesResponse`
pub const LIST_STATUSES_PATH: &str = "/api/v1/ci-integration/status/{owner}/{repo}/{sha}";
pub const LIST_STATUSES_METHOD: &str = "GET";

/// Response for listing status checks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListStatusesResponse {
    /// The commit SHA.
    pub commit_sha: String,
    /// All status checks for this commit.
    pub statuses: Vec<GitHubStatus>,
    /// Count of status checks.
    pub count: u32,
}

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/ci-integration/comments
// ---------------------------------------------------------------------------

/// POST /api/v1/ci-integration/comments
///
/// Post or update an execution summary comment on a PR/issue.
///
/// **Request Body:** `UpsertCommentRequest`
/// **Response:** `201 Created` with `CommentResponse`
pub const UPSERT_COMMENT_PATH: &str = "/api/v1/ci-integration/comments";
pub const UPSERT_COMMENT_METHOD: &str = "POST";

/// Request body for upserting an execution summary comment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpsertCommentRequest {
    /// Repository owner.
    pub owner: String,
    /// Repository name.
    pub repo: String,
    /// The issue or PR number.
    pub issue_number: u64,
    /// The execution summary to post.
    pub summary: ExecutionSummary,
}

/// Response from upserting a comment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommentResponse {
    /// The GitHub comment ID.
    pub comment_id: u64,
    /// Whether the comment was created (true) or updated (false).
    pub created: bool,
}

// ---------------------------------------------------------------------------
// Endpoint: GET /api/v1/ci-integration/comments/{owner}/{repo}/{issue}
// ---------------------------------------------------------------------------

/// GET /api/v1/ci-integration/comments/{owner}/{repo}/{issue}
///
/// List all comments on an issue/PR.
///
/// **Response:** `200 OK` with `ListCommentsResponse`
pub const LIST_COMMENTS_PATH: &str =
    "/api/v1/ci-integration/comments/{owner}/{repo}/{issue_number}";
pub const LIST_COMMENTS_METHOD: &str = "GET";

/// Response for listing comments.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListCommentsResponse {
    /// The issue/PR number.
    pub issue_number: u64,
    /// All comments on this issue/PR.
    pub comments: Vec<PrComment>,
    /// Count of comments.
    pub count: u32,
}

// ---------------------------------------------------------------------------
// Endpoint: GET /api/v1/ci-integration/bot-comment/{owner}/{repo}/{issue}
// ---------------------------------------------------------------------------

/// GET /api/v1/ci-integration/bot-comment/{owner}/{repo}/{issue}
///
/// Find the rigorix bot comment on an issue/PR.
///
/// **Response:** `200 OK` with `BotCommentResponse` (or `404` if not found)
pub const FIND_BOT_COMMENT_PATH: &str =
    "/api/v1/ci-integration/bot-comment/{owner}/{repo}/{issue_number}";
pub const FIND_BOT_COMMENT_METHOD: &str = "GET";

/// Response for finding the bot comment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BotCommentResponse {
    /// The bot comment, if found.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comment: Option<PrComment>,
    /// Whether the bot comment was found.
    pub found: bool,
}

// ---------------------------------------------------------------------------
// Endpoint: GET /api/v1/ci-integration/health
// ---------------------------------------------------------------------------

/// GET /api/v1/ci-integration/health
///
/// Health check for the CI integration module.
///
/// **Response:** `200 OK` with `HealthResponse`
pub const HEALTH_PATH: &str = "/api/v1/ci-integration/health";
pub const HEALTH_METHOD: &str = "GET";

/// Health check response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    /// Whether the module is healthy.
    pub healthy: bool,
    /// Module version.
    pub version: &'static str,
    /// Whether the GitHub API client is configured.
    pub github_client_configured: bool,
}
