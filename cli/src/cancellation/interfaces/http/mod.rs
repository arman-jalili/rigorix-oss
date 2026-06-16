//! HTTP API contracts for CLI Cancellation endpoints.
//!
//! @canonical .pi/architecture/modules/cancellation.md
//! Implements: Contract Freeze — HTTP endpoint contracts and error formats
//! Issue: issue-contract-freeze
//!
//! Defines endpoint paths, methods, request/response schemas, and error
//! response formats for CLI-to-engine cancellation operations. These contracts
//! are framework-agnostic — they describe the API surface that any HTTP
//! server implementation must satisfy.
//!
//! The CLI cancellation module exposes operations for:
//! - Querying signal handler status
//! - Requesting graceful shutdown
//! - Requesting immediate abort
//!
//! # Contract (Frozen)
//! - All endpoints documented with method, path, request, and response types
//! - Error responses follow a unified format
//! - No framework-specific annotations (axum/actix/warp annotations added by implementation)

use serde::{Deserialize, Serialize};

use crate::cancellation::application::dto::{
    GracefulShutdownInput, ImmediateShutdownInput, ShutdownOutput, SignalStatusOutput,
};

// ---------------------------------------------------------------------------
// API Base Path
// ---------------------------------------------------------------------------

/// All CLI cancellation endpoints are served under this base path.
pub const API_BASE_PATH: &str = "/api/v1/cli/cancellation";

// ---------------------------------------------------------------------------
// Endpoint: GET /api/v1/cli/cancellation/status
// ---------------------------------------------------------------------------

/// GET /api/v1/cli/cancellation/status
///
/// Get the current signal handler status.
///
/// **Response:** `200 OK` with `CancellationStatusResponse`
pub const CANCELLATION_STATUS_PATH: &str = "/api/v1/cli/cancellation/status";
pub const CANCELLATION_STATUS_METHOD: &str = "GET";

/// Response for GET /api/v1/cli/cancellation/status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CancellationStatusResponse {
    /// Whether the signal handler is installed.
    pub installed: bool,
    /// The current shutdown level.
    pub current_level: String,
    /// The double-press window in seconds.
    pub double_press_window_secs: u64,
    /// Timestamp of the last signal received.
    pub last_signal_at: Option<String>,
}

impl From<SignalStatusOutput> for CancellationStatusResponse {
    fn from(output: SignalStatusOutput) -> Self {
        Self {
            installed: output.installed,
            current_level: format!("{:?}", output.current_level),
            double_press_window_secs: output.double_press_window_secs,
            last_signal_at: output.last_signal_at,
        }
    }
}

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/cli/cancellation/shutdown/graceful
// ---------------------------------------------------------------------------

/// POST /api/v1/cli/cancellation/shutdown/graceful
///
/// Request a graceful shutdown.
///
/// **Request:** `GracefulShutdownApiRequest`
/// **Response:** `200 OK` with `ShutdownApiResponse`
pub const GRACEFUL_SHUTDOWN_PATH: &str = "/api/v1/cli/cancellation/shutdown/graceful";
pub const GRACEFUL_SHUTDOWN_METHOD: &str = "POST";

/// Request body for POST /api/v1/cli/cancellation/shutdown/graceful.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GracefulShutdownApiRequest {
    /// Human-readable reason for the shutdown.
    pub reason: Option<String>,
    /// Timeout in seconds for in-flight tasks.
    #[serde(default = "default_shutdown_timeout")]
    pub timeout_secs: u64,
}

fn default_shutdown_timeout() -> u64 {
    30
}

impl From<GracefulShutdownApiRequest> for GracefulShutdownInput {
    fn from(req: GracefulShutdownApiRequest) -> Self {
        Self {
            reason: req.reason,
            timeout_secs: req.timeout_secs,
        }
    }
}

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/cli/cancellation/shutdown/immediate
// ---------------------------------------------------------------------------

/// POST /api/v1/cli/cancellation/shutdown/immediate
///
/// Request an immediate abort.
///
/// **Request:** `ImmediateShutdownApiRequest`
/// **Response:** `200 OK` with `ShutdownApiResponse`
pub const IMMEDIATE_SHUTDOWN_PATH: &str = "/api/v1/cli/cancellation/shutdown/immediate";
pub const IMMEDIATE_SHUTDOWN_METHOD: &str = "POST";

/// Request body for POST /api/v1/cli/cancellation/shutdown/immediate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImmediateShutdownApiRequest {
    /// Human-readable reason for the abort.
    pub reason: Option<String>,
}

impl From<ImmediateShutdownApiRequest> for ImmediateShutdownInput {
    fn from(req: ImmediateShutdownApiRequest) -> Self {
        Self { reason: req.reason }
    }
}

// ---------------------------------------------------------------------------
// Response DTOs
// ---------------------------------------------------------------------------

/// Response for shutdown API requests.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShutdownApiResponse {
    pub success: bool,
    pub tasks_cancelled: u32,
    pub duration_ms: u64,
}

impl From<ShutdownOutput> for ShutdownApiResponse {
    fn from(output: ShutdownOutput) -> Self {
        Self {
            success: output.success,
            tasks_cancelled: output.tasks_cancelled,
            duration_ms: output.duration_ms,
        }
    }
}

// ---------------------------------------------------------------------------
// Unified Error Response Format
// ---------------------------------------------------------------------------

/// Standard error response for CLI Cancellation API endpoints.
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

/// Standardized error codes for CLI Cancellation API.
pub mod error_codes {
    /// Signal handler not installed.
    pub const NOT_INSTALLED: &str = "CANCELLATION_NOT_INSTALLED";
    /// Signal handler installation failed.
    pub const INSTALL_FAILED: &str = "CANCELLATION_INSTALL_FAILED";
    /// Shutdown already in progress.
    pub const ALREADY_SHUTTING_DOWN: &str = "CANCELLATION_ALREADY_SHUTTING_DOWN";
    /// Internal server error.
    pub const INTERNAL_ERROR: &str = "CANCELLATION_INTERNAL_ERROR";
}

/// HTTP status code mappings for CLI Cancellation errors.
pub mod status_codes {
    pub const NOT_INSTALLED: u16 = 503;
    pub const INSTALL_FAILED: u16 = 500;
    pub const ALREADY_SHUTTING_DOWN: u16 = 409;
    pub const INTERNAL_ERROR: u16 = 500;
}
