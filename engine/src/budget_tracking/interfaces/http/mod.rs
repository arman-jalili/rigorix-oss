//! HTTP API contracts for Budget Tracking endpoints.
//!
//! @canonical .pi/architecture/modules/budget-tracking.md
//! Implements: Contract Freeze — HTTP endpoint contracts and error formats
//! Issue: #68
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

use crate::budget_tracking::application::dto::{
    BudgetWarningInfo, CommitReservationInput, CommitReservationOutput, GetBudgetStatusOutput,
    ReserveBudgetInput, ReserveBudgetOutput,
};

// ---------------------------------------------------------------------------
// API Base Path
// ---------------------------------------------------------------------------

/// All budget tracking endpoints are served under this base path.
pub const API_BASE_PATH: &str = "/api/v1/budget";

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/budget/reserve
// ---------------------------------------------------------------------------

/// POST /api/v1/budget/reserve
///
/// Reserve budget for an LLM call.
///
/// **Request:** `ReserveBudgetRequest`
/// **Response:** `201 Created` with `ReserveBudgetResponse`
pub const RESERVE_BUDGET_PATH: &str = "/api/v1/budget/reserve";
pub const RESERVE_BUDGET_METHOD: &str = "POST";

/// Request body for POST /api/v1/budget/reserve.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReserveBudgetRequest {
    /// The execution ID to associate this reservation with.
    pub execution_id: uuid::Uuid,

    /// Estimated number of tokens the LLM call will consume.
    ///
    /// Must be > 0.
    pub estimated_tokens: u32,

    /// Optional label for this specific call (e.g., "classify", "extract").
    pub call_label: Option<String>,
}

impl From<ReserveBudgetRequest> for ReserveBudgetInput {
    fn from(req: ReserveBudgetRequest) -> Self {
        Self {
            execution_id: req.execution_id,
            estimated_tokens: req.estimated_tokens,
            call_label: req.call_label,
        }
    }
}

/// Response body for POST /api/v1/budget/reserve.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReserveBudgetResponse {
    pub success: bool,
    pub call_id: u32,
    pub reserved_tokens: u32,
    pub remaining_calls: u32,
    pub remaining_tokens: u32,
    pub calls_used: u32,
    pub tokens_used_before_reservation: u32,
}

impl From<ReserveBudgetOutput> for ReserveBudgetResponse {
    fn from(output: ReserveBudgetOutput) -> Self {
        Self {
            success: true,
            call_id: output.reservation.call_id,
            reserved_tokens: output.reservation.reserved_tokens,
            remaining_calls: output.remaining_calls,
            remaining_tokens: output.remaining_tokens,
            calls_used: output.calls_used,
            tokens_used_before_reservation: output.tokens_used_before_reservation,
        }
    }
}

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/budget/commit
// ---------------------------------------------------------------------------

/// POST /api/v1/budget/commit
///
/// Commit a reservation with actual token consumption.
///
/// **Request:** `CommitReservationRequest`
/// **Response:** `200 OK` with `CommitReservationResponse`
pub const COMMIT_BUDGET_PATH: &str = "/api/v1/budget/commit";
pub const COMMIT_BUDGET_METHOD: &str = "POST";

/// Request body for POST /api/v1/budget/commit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitReservationRequest {
    /// The execution ID this reservation belongs to.
    pub execution_id: uuid::Uuid,

    /// The call identifier from the reservation.
    pub call_id: u32,

    /// Actual number of tokens consumed by the LLM call.
    pub actual_tokens: u32,
}

impl From<CommitReservationRequest> for CommitReservationInput {
    fn from(req: CommitReservationRequest) -> Self {
        Self {
            execution_id: req.execution_id,
            call_id: req.call_id,
            actual_tokens: req.actual_tokens,
        }
    }
}

/// Response body for POST /api/v1/budget/commit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitReservationResponse {
    pub success: bool,
    pub call_id: u32,
    pub actual_tokens: u32,
    pub remaining_calls: u32,
    pub remaining_tokens: u32,
    pub total_calls_used: u32,
    pub total_tokens_used: u32,
    pub warnings: Vec<BudgetWarningInfoDto>,
}

impl From<CommitReservationOutput> for CommitReservationResponse {
    fn from(output: CommitReservationOutput) -> Self {
        Self {
            success: true,
            call_id: output.reservation.call_id,
            actual_tokens: output.reservation.actual_tokens.unwrap_or(0),
            remaining_calls: output.remaining_calls,
            remaining_tokens: output.remaining_tokens,
            total_calls_used: output.total_calls_used,
            total_tokens_used: output.total_tokens_used,
            warnings: output
                .warnings_triggered
                .into_iter()
                .map(BudgetWarningInfoDto::from)
                .collect(),
        }
    }
}

// ---------------------------------------------------------------------------
// Endpoint: GET /api/v1/budget/status
// ---------------------------------------------------------------------------

/// GET /api/v1/budget/status
///
/// Get the current budget status for a given execution.
///
/// **Path parameter:** `execution_id` (UUID)
/// **Response:** `200 OK` with `BudgetStatusResponse`
pub const BUDGET_STATUS_PATH: &str = "/api/v1/budget/status/{execution_id}";
pub const BUDGET_STATUS_METHOD: &str = "GET";

/// Response body for GET /api/v1/budget/status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetStatusResponse {
    pub success: bool,
    pub label: String,
    pub max_calls: u32,
    pub max_tokens: u32,
    pub calls_used: u32,
    pub tokens_used: u32,
    pub remaining_calls: u32,
    pub remaining_tokens: u32,
    pub call_usage_ratio: f64,
    pub token_usage_ratio: f64,
    pub active_warnings: Vec<BudgetWarningInfoDto>,
}

impl From<GetBudgetStatusOutput> for BudgetStatusResponse {
    fn from(output: GetBudgetStatusOutput) -> Self {
        Self {
            success: true,
            label: output.label,
            max_calls: output.max_calls,
            max_tokens: output.max_tokens,
            calls_used: output.calls_used,
            tokens_used: output.tokens_used,
            remaining_calls: output.remaining_calls,
            remaining_tokens: output.remaining_tokens,
            call_usage_ratio: output.call_usage_ratio,
            token_usage_ratio: output.token_usage_ratio,
            active_warnings: output
                .active_warnings
                .into_iter()
                .map(BudgetWarningInfoDto::from)
                .collect(),
        }
    }
}

// ---------------------------------------------------------------------------
// Endpoint: GET /api/v1/budget/presets
// ---------------------------------------------------------------------------

/// GET /api/v1/budget/presets
///
/// List available budget presets with their limits.
///
/// **Response:** `200 OK` with `BudgetPresetsResponse`
pub const BUDGET_PRESETS_PATH: &str = "/api/v1/budget/presets";
pub const BUDGET_PRESETS_METHOD: &str = "GET";

/// Response body for GET /api/v1/budget/presets.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetPresetsResponse {
    pub presets: Vec<BudgetPresetDto>,
}

/// DTO for a budget preset description.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetPresetDto {
    pub label: String,
    pub max_calls: u32,
    pub max_tokens: u32,
    pub description: String,
}

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/budget/reset
// ---------------------------------------------------------------------------

/// POST /api/v1/budget/reset
///
/// Reset the budget for a given execution back to zero usage.
///
/// **Request:** `ResetBudgetRequest`
/// **Response:** `200 OK` with `ResetBudgetResponse`
pub const RESET_BUDGET_PATH: &str = "/api/v1/budget/reset";
pub const RESET_BUDGET_METHOD: &str = "POST";

/// Request body for POST /api/v1/budget/reset.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResetBudgetRequest {
    /// The execution ID whose budget should be reset.
    pub execution_id: uuid::Uuid,
}

/// Response body for POST /api/v1/budget/reset.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResetBudgetResponse {
    pub success: bool,
}

// ---------------------------------------------------------------------------
// Shared DTOs
// ---------------------------------------------------------------------------

/// DTO for budget warning information in API responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetWarningInfoDto {
    pub resource: String,
    pub used: u32,
    pub max: u32,
    pub usage_ratio: f64,
    pub is_exhausted: bool,
}

impl From<BudgetWarningInfo> for BudgetWarningInfoDto {
    fn from(info: BudgetWarningInfo) -> Self {
        Self {
            resource: info.resource,
            used: info.used,
            max: info.max,
            usage_ratio: info.usage_ratio,
            is_exhausted: info.is_exhausted,
        }
    }
}

// ---------------------------------------------------------------------------
// Unified Error Response Format
// ---------------------------------------------------------------------------

/// Standard error response for all Budget Tracking API endpoints.
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

/// Standardized error codes for Budget Tracking API.
pub mod error_codes {
    /// Maximum LLM calls exceeded.
    pub const MAX_CALLS_EXCEEDED: &str = "BUDGET_MAX_CALLS_EXCEEDED";
    /// Maximum tokens exceeded.
    pub const MAX_TOKENS_EXCEEDED: &str = "BUDGET_MAX_TOKENS_EXCEEDED";
    /// Budget reservation failed.
    pub const RESERVATION_FAILED: &str = "BUDGET_RESERVATION_FAILED";
    /// Budget not initialized.
    pub const NOT_INITIALIZED: &str = "BUDGET_NOT_INITIALIZED";
    /// Internal server error.
    pub const INTERNAL_ERROR: &str = "BUDGET_INTERNAL_ERROR";
}

/// HTTP status code mappings for Budget Tracking errors.
pub mod status_codes {
    pub const MAX_CALLS_EXCEEDED: u16 = 429;
    pub const MAX_TOKENS_EXCEEDED: u16 = 429;
    pub const RESERVATION_FAILED: u16 = 400;
    pub const NOT_INITIALIZED: u16 = 503;
    pub const INTERNAL_ERROR: u16 = 500;
}
