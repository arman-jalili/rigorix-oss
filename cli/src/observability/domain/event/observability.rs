//! Observability event payload schemas for the CLI boundary.
//!
//! @canonical .pi/architecture/modules/observability.md#events
//! Implements: Contract Freeze — Observability event schemas
//! Issue: #253
//!
//! Events emitted by the CLI observability layer. These represent
//! lifecycle events for tracing, health checks, and metrics.
//!
//! # Contract (Frozen)
//! - Each variant is a serializable struct with derived Debug
//! - Variants are additive only (no removal without architecture review)
//! - All events carry at least a timestamp

use serde::{Deserialize, Serialize};

/// Events emitted by the observability layer.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum ObservabilityEvent {
    /// Tracing was initialized.
    TracingInitialized(TracingInitializedPayload),

    /// A health check was performed.
    HealthCheckPerformed(HealthCheckPayload),

    /// A health check component registered a status change.
    HealthStatusChanged(HealthStatusChangedPayload),
}

/// Payload for tracing initialization events.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TracingInitializedPayload {
    /// The log level that was configured.
    pub log_level: String,
    /// The log format (pretty or json).
    pub log_format: String,
    /// Whether RIGORIX_LOG env var was set.
    pub env_override: bool,
    /// Wall-clock timestamp as ISO 8601.
    pub timestamp: String,
}

/// The health status of a component.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HealthStatus {
    /// Component is functioning normally.
    #[serde(rename = "healthy")]
    Healthy,
    /// Component is functioning but degraded (e.g., high latency).
    #[serde(rename = "degraded")]
    Degraded,
    /// Component is not functioning.
    #[serde(rename = "unhealthy")]
    Unhealthy,
}

/// Payload for health check events.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckPayload {
    /// Overall health status.
    pub status: HealthStatus,
    /// Number of healthy components.
    pub healthy_count: u32,
    /// Number of degraded components.
    pub degraded_count: u32,
    /// Number of unhealthy components.
    pub unhealthy_count: u32,
    /// Wall-clock timestamp as ISO 8601.
    pub timestamp: String,
}

/// Payload for health status change events.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthStatusChangedPayload {
    /// The component name.
    pub component: String,
    /// The previous health status.
    pub previous: HealthStatus,
    /// The current health status.
    pub current: HealthStatus,
    /// A human-readable message describing the change.
    pub message: String,
    /// Wall-clock timestamp as ISO 8601.
    pub timestamp: String,
}
