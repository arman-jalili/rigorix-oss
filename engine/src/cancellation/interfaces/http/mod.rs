//! HTTP API contracts for Cancellation endpoints.
//!
//! @canonical .pi/architecture/modules/cancellation.md
//! Implements: Contract Freeze — HTTP endpoint contracts and error formats
//! Issue: issue-contract-freeze
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

use crate::cancellation::application::dto::{
    CancelExecutionOutput, ShutdownOutput, ShutdownStatusOutput,
};

use crate::cancellation::domain::ShutdownSignal;

// ---------------------------------------------------------------------------
// API Base Path
// ---------------------------------------------------------------------------

/// All cancellation endpoints are served under this base path.
pub const API_BASE_PATH: &str = "/api/v1/execution";

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/execution/{id}/cancel
// ---------------------------------------------------------------------------

/// POST /api/v1/execution/{id}/cancel
///
/// Request graceful cancellation of a running execution.
/// Running tasks finish naturally; no new tasks are started.
///
/// **Path Param:** `id` — Execution UUID
/// **Request:** `CancelExecutionRequest`
/// **Response:** `202 Accepted` with `CancelExecutionResponse`
pub const CANCEL_EXECUTION_PATH: &str = "/api/v1/execution/{id}/cancel";
pub const CANCEL_EXECUTION_METHOD: &str = "POST";

/// Request body for POST /api/v1/execution/{id}/cancel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CancelExecutionRequest {
    /// Human-readable reason for cancellation.
    pub reason: Option<String>,

    /// Source identifier for the cancellation request.
    /// Default: "api"
    pub source: Option<String>,

    /// Whether this is an immediate abort instead of graceful.
    /// Default: false (graceful)
    pub immediate: Option<bool>,
}

/// Response body for POST /api/v1/execution/{id}/cancel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CancelExecutionResponse {
    pub accepted: bool,
    pub signal: ShutdownSignal,
    pub affected_tasks: u32,
    pub was_already_cancelling: bool,
    pub message: String,
}

impl From<CancelExecutionOutput> for CancelExecutionResponse {
    fn from(output: CancelExecutionOutput) -> Self {
        let message = if output.was_already_cancelling {
            format!(
                "Cancellation already in progress with signal {:?}",
                output.signal
            )
        } else {
            format!(
                "Cancellation requested with {:?} signal ({} tasks affected)",
                output.signal, output.affected_tasks
            )
        };

        Self {
            accepted: output.accepted,
            signal: output.signal,
            affected_tasks: output.affected_tasks,
            was_already_cancelling: output.was_already_cancelling,
            message,
        }
    }
}

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/execution/{id}/abort
// ---------------------------------------------------------------------------

/// POST /api/v1/execution/{id}/abort
///
/// Immediately abort a running execution. All in-flight work is
/// terminated. Use with caution — resources may be left in an
/// inconsistent state.
///
/// **Path Param:** `id` — Execution UUID
/// **Request:** `AbortExecutionRequest`
/// **Response:** `202 Accepted` with `CancelExecutionResponse`
pub const ABORT_EXECUTION_PATH: &str = "/api/v1/execution/{id}/abort";
pub const ABORT_EXECUTION_METHOD: &str = "POST";

/// Request body for POST /api/v1/execution/{id}/abort.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AbortExecutionRequest {
    /// Reason for the abort.
    pub reason: Option<String>,

    /// Source identifier.
    /// Default: "api"
    pub source: Option<String>,
}

// Both abort and cancel share the same response type.
pub use CancelExecutionResponse as AbortExecutionResponse;

// ---------------------------------------------------------------------------
// Endpoint: GET /api/v1/execution/{id}/status
// ---------------------------------------------------------------------------

/// GET /api/v1/execution/{id}/status
///
/// Get the current cancellation status of an execution.
///
/// **Path Param:** `id` — Execution UUID
/// **Response:** `200 OK` with `ExecutionStatusResponse`
pub const EXECUTION_STATUS_PATH: &str = "/api/v1/execution/{id}/status";
pub const EXECUTION_STATUS_METHOD: &str = "GET";

/// Response body for GET /api/v1/execution/{id}/status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionStatusResponse {
    pub execution_id: String,
    pub is_cancelled: bool,
    pub current_signal: Option<ShutdownSignal>,
    pub running_tasks: u32,
    pub completed_tasks: u32,
    pub cancelled_tasks: u32,
    pub shutdown_complete: bool,
    pub elapsed_since_request_ms: Option<u64>,
}

impl From<ShutdownStatusOutput> for ExecutionStatusResponse {
    fn from(output: ShutdownStatusOutput) -> Self {
        Self {
            execution_id: String::new(), // Populated by the handler
            is_cancelled: output.is_cancelled,
            current_signal: output.current_signal,
            running_tasks: output.running_tasks,
            completed_tasks: output.completed_tasks,
            cancelled_tasks: output.cancelled_tasks,
            shutdown_complete: output.shutdown_complete,
            elapsed_since_request_ms: output.elapsed_since_request_ms,
        }
    }
}

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/execution/{id}/await-shutdown
// ---------------------------------------------------------------------------

/// POST /api/v1/execution/{id}/await-shutdown
///
/// Wait for shutdown to complete on a cancelled execution.
/// Blocks until all tasks finish or the specified timeout is reached.
///
/// **Path Param:** `id` — Execution UUID
/// **Request:** `AwaitShutdownRequest`
/// **Response:** `200 OK` with `AwaitShutdownResponse`
pub const AWAIT_SHUTDOWN_PATH: &str = "/api/v1/execution/{id}/await-shutdown";
pub const AWAIT_SHUTDOWN_METHOD: &str = "POST";

/// Request body for POST /api/v1/execution/{id}/await-shutdown.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AwaitShutdownRequest {
    /// Timeout in seconds (default: 30).
    pub timeout_secs: Option<u64>,

    /// Force-abort remaining tasks after timeout (default: true).
    pub force_abort_on_timeout: Option<bool>,
}

/// Response body for POST /api/v1/execution/{id}/await-shutdown.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AwaitShutdownResponse {
    pub execution_id: String,
    pub signal_used: ShutdownSignal,
    pub total_tasks: u32,
    pub completed_tasks: u32,
    pub cancelled_tasks: u32,
    pub shutdown_duration_ms: u64,
    pub forced: bool,
    pub cleanup_success: bool,
    pub timed_out: bool,
}

impl From<ShutdownOutput> for AwaitShutdownResponse {
    fn from(output: ShutdownOutput) -> Self {
        Self {
            execution_id: String::new(), // Populated by the handler
            signal_used: output.signal_used,
            total_tasks: output.total_tasks,
            completed_tasks: output.completed_tasks,
            cancelled_tasks: output.cancelled_tasks,
            shutdown_duration_ms: output.shutdown_duration_ms,
            forced: output.forced,
            cleanup_success: output.cleanup_success,
            timed_out: false,
        }
    }
}

// ---------------------------------------------------------------------------
// Unified Error Response Format
// ---------------------------------------------------------------------------

/// Standard error response for all Cancellation API endpoints.
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

/// Standardized error codes for Cancellation API.
pub mod error_codes {
    /// Execution not found.
    pub const EXECUTION_NOT_FOUND: &str = "EXECUTION_NOT_FOUND";
    /// Task not found.
    pub const TASK_NOT_FOUND: &str = "TASK_NOT_FOUND";
    /// Already cancelled.
    pub const ALREADY_CANCELLED: &str = "ALREADY_CANCELLED";
    /// Shutdown timed out.
    pub const SHUTDOWN_TIMEOUT: &str = "SHUTDOWN_TIMEOUT";
    /// No active execution.
    pub const NO_ACTIVE_EXECUTION: &str = "NO_ACTIVE_EXECUTION";
    /// Internal server error.
    pub const INTERNAL_ERROR: &str = "CANCELLATION_INTERNAL_ERROR";
}

/// HTTP status code mappings for Cancellation errors.
pub mod status_codes {
    pub const EXECUTION_NOT_FOUND: u16 = 404;
    pub const TASK_NOT_FOUND: u16 = 404;
    pub const ALREADY_CANCELLED: u16 = 409;
    pub const SHUTDOWN_TIMEOUT: u16 = 504;
    pub const NO_ACTIVE_EXECUTION: u16 = 404;
    pub const INTERNAL_ERROR: u16 = 500;
}
