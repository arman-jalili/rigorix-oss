//! Data Transfer Objects for the CLI Observability module.
//!
//! @canonical .pi/architecture/modules/observability.md
//! Implements: Contract Freeze — CLI observability DTO schemas
//! Issue: issue-contract-freeze
//!
//! DTOs define the input/output contracts for observability operations.
//!
//! # Contract (Frozen)
//! - Every service operation has a dedicated input and output DTO
//! - DTOs are serializable (JSON for CI/CD output)
//! - Fields use reasonable Rust types

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Tracing DTOs
// ---------------------------------------------------------------------------

/// Input for initializing tracing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitTracingInput {
    /// Minimum log level.
    pub log_level: String,
    /// Output format (pretty, json).
    pub log_format: String,
}

/// Output from initializing tracing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitTracingOutput {
    /// Whether tracing was successfully initialized.
    pub success: bool,
    /// Whether this was the first initialization or a no-op.
    pub initialized: bool,
}

// ---------------------------------------------------------------------------
// Health Check DTOs
// ---------------------------------------------------------------------------

/// Input for running health checks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckInput;

/// Output from running health checks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckOutput {
    /// Overall health status.
    pub healthy: bool,
    /// Individual health check results.
    pub checks: Vec<HealthCheckResult>,
    /// Number of checks that passed.
    pub passed: u32,
    /// Number of checks that failed.
    pub failed: u32,
}

/// Result of a single health check.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckResult {
    /// The name of the health check.
    pub name: String,
    /// Whether this check passed.
    pub healthy: bool,
    /// Optional error message if the check failed.
    pub error: Option<String>,
    /// Duration of the check in milliseconds.
    pub duration_ms: u64,
}

// ---------------------------------------------------------------------------
// Metrics DTOs
// ---------------------------------------------------------------------------

/// A single metric value for reporting.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricValue {
    /// The metric name.
    pub name: String,
    /// The metric value.
    pub value: f64,
    /// Optional metric labels.
    pub labels: Vec<(String, String)>,
}

/// Output from collecting metrics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsOutput {
    /// All collected metrics.
    pub metrics: Vec<MetricValue>,
    /// Timestamp of collection.
    pub timestamp: String,
}
