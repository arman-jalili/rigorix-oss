//! HTTP API contracts for CLI Observability endpoints.
//!
//! @canonical .pi/architecture/modules/observability.md
//! Implements: Contract Freeze — HTTP endpoint contracts and error formats
//! Issue: issue-contract-freeze
//!
//! Defines endpoint paths, methods, request/response schemas for CLI
//! observability operations (tracing status, health checks, metrics).
//!
//! # Contract (Frozen)
//! - All endpoints documented with method, path, request, and response types
//! - Error responses follow a unified format
//! - No framework-specific annotations

use serde::{Deserialize, Serialize};

use crate::observability::application::dto::{HealthCheckOutput, InitTracingOutput, MetricsOutput};

// ---------------------------------------------------------------------------
// API Base Path
// ---------------------------------------------------------------------------

pub const API_BASE_PATH: &str = "/api/v1/cli/observability";

// ---------------------------------------------------------------------------
// Endpoint: GET /api/v1/cli/observability/health
// ---------------------------------------------------------------------------

pub const HEALTH_PATH: &str = "/api/v1/cli/observability/health";
pub const HEALTH_METHOD: &str = "GET";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    pub healthy: bool,
    pub checks: Vec<HealthCheckItemResponse>,
    pub passed: u32,
    pub failed: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckItemResponse {
    pub name: String,
    pub healthy: bool,
    pub error: Option<String>,
    pub duration_ms: u64,
}

impl From<HealthCheckOutput> for HealthResponse {
    fn from(o: HealthCheckOutput) -> Self {
        Self {
            healthy: o.healthy,
            checks: o
                .checks
                .into_iter()
                .map(|c| HealthCheckItemResponse {
                    name: c.name,
                    healthy: c.healthy,
                    error: c.error,
                    duration_ms: c.duration_ms,
                })
                .collect(),
            passed: o.passed,
            failed: o.failed,
        }
    }
}

// ---------------------------------------------------------------------------
// Endpoint: GET /api/v1/cli/observability/tracing/status
// ---------------------------------------------------------------------------

pub const TRACING_STATUS_PATH: &str = "/api/v1/cli/observability/tracing/status";
pub const TRACING_STATUS_METHOD: &str = "GET";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TracingStatusResponse {
    pub initialized: bool,
    pub log_level: Option<String>,
    pub log_format: Option<String>,
}

impl From<InitTracingOutput> for TracingStatusResponse {
    fn from(o: InitTracingOutput) -> Self {
        Self {
            initialized: o.initialized,
            log_level: None,
            log_format: None,
        }
    }
}

// ---------------------------------------------------------------------------
// Endpoint: GET /api/v1/cli/observability/metrics
// ---------------------------------------------------------------------------

pub const METRICS_PATH: &str = "/api/v1/cli/observability/metrics";
pub const METRICS_METHOD: &str = "GET";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsResponse {
    pub metrics: Vec<MetricItemResponse>,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricItemResponse {
    pub name: String,
    pub value: f64,
    pub labels: Vec<Vec<String>>,
}

impl From<MetricsOutput> for MetricsResponse {
    fn from(o: MetricsOutput) -> Self {
        Self {
            metrics: o
                .metrics
                .into_iter()
                .map(|m| MetricItemResponse {
                    name: m.name,
                    value: m.value,
                    labels: m.labels.into_iter().map(|(k, v)| vec![k, v]).collect(),
                })
                .collect(),
            timestamp: o.timestamp,
        }
    }
}

// ---------------------------------------------------------------------------
// Unified Error Response Format
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CliApiErrorResponse {
    pub status: u16,
    pub code: String,
    pub message: String,
    pub details: Option<serde_json::Value>,
    pub request_id: Option<String>,
}

pub mod error_codes {
    pub const TRACING_NOT_INITIALIZED: &str = "OBSERVABILITY_TRACING_NOT_INIT";
    pub const HEALTH_CHECK_FAILED: &str = "OBSERVABILITY_HEALTH_FAILED";
    pub const INTERNAL_ERROR: &str = "OBSERVABILITY_INTERNAL_ERROR";
}

pub mod status_codes {
    pub const TRACING_NOT_INITIALIZED: u16 = 503;
    pub const HEALTH_CHECK_FAILED: u16 = 500;
    pub const INTERNAL_ERROR: u16 = 500;
}
