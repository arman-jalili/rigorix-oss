//! Health check trait and types.
//!
//! @canonical .pi/architecture/modules/observability.md#health-check

use serde::Serialize;
// Only `check_health_with_timeout`'s default body takes a `Duration`, and it is compiled out on
// wasm32 — a single-threaded module has no timer (#1400). The import follows it rather than
// warning on the target that ships.
#[cfg(not(target_arch = "wasm32"))]
use std::time::Duration;

/// The health status of a single component.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum HealthStatus {
    /// The component is functioning normally.
    Healthy,
    /// The component is degraded but still operational.
    Degraded,
    /// The component is not functioning.
    Unhealthy,
}

impl HealthStatus {
    pub fn is_healthy(&self) -> bool {
        matches!(self, HealthStatus::Healthy)
    }

    pub fn is_terminal(&self) -> bool {
        matches!(self, HealthStatus::Unhealthy)
    }
}

/// A health report for a single component.
#[derive(Debug, Clone, Serialize)]
pub struct HealthReport {
    /// Name of the component.
    pub component: String,
    /// Current health status.
    pub status: HealthStatus,
    /// Human-readable description of the component's state.
    pub message: String,
    /// Unix timestamp of the last successful activity.
    pub last_activity_at: Option<i64>,
    /// Time taken to run this health check in milliseconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
}

impl HealthReport {
    pub fn healthy(component: impl Into<String>) -> Self {
        Self {
            component: component.into(),
            status: HealthStatus::Healthy,
            message: "OK".to_string(),
            last_activity_at: None,
            duration_ms: None,
        }
    }

    pub fn degraded(component: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            component: component.into(),
            status: HealthStatus::Degraded,
            message: message.into(),
            last_activity_at: None,
            duration_ms: None,
        }
    }

    pub fn unhealthy(component: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            component: component.into(),
            status: HealthStatus::Unhealthy,
            message: message.into(),
            last_activity_at: None,
            duration_ms: None,
        }
    }
}

/// A health check for a single component.
#[async_trait::async_trait]
pub trait HealthCheck: Send + Sync {
    /// The name of this component.
    fn component_name(&self) -> &str;

    /// Run the health check and return a report.
    async fn check_health(&self) -> HealthReport;

    /// Run the health check with a timeout.
    ///
    /// # Not on wasm32
    ///
    /// This default body is the only place on the gate path that needed a *timer*:
    /// `tokio::select!` plus `tokio::time::sleep` plus `std::time::Instant`. A single-threaded
    /// module has no runtime to drive either, and no component whose health it would probe —
    /// so it is compiled out there rather than kept alive by widening the tokio feature list
    /// (#1400). `tokio` on wasm32 is now `sync` alone: no `rt`, no `time`, no `macros`.
    #[cfg(not(target_arch = "wasm32"))]
    async fn check_health_with_timeout(&self, timeout: Duration) -> HealthReport {
        let start = std::time::Instant::now();
        let check = self.check_health();
        tokio::select! {
            report = check => {
                let mut report = report;
                report.duration_ms = Some(start.elapsed().as_millis() as u64);
                report
            }
            _ = tokio::time::sleep(timeout) => {
                HealthReport::unhealthy(
                    self.component_name(),
                    format!("Health check timed out after {}ms", timeout.as_millis()),
                )
            }
        }
    }
}
