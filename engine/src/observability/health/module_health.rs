//! Pre-built HealthCheck implementations for all Rigorix modules.
//!
//! @canonical .pi/architecture/modules/observability.md#module-health
//!
//! Each module exports a health check struct that implements the `HealthCheck`
//! trait. These can be registered with the centralized `HealthService` to
//! provide per-component health visibility.

use super::health_check::{HealthCheck, HealthReport, HealthStatus};
use async_trait::async_trait;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;

/// A simple health check that reports a component's status and key metric.
pub struct SimpleHealthCheck {
    name: String,
    last_activity: AtomicI64,
}

impl SimpleHealthCheck {
    /// Create a new simple health check.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            last_activity: AtomicI64::new(chrono::Utc::now().timestamp()),
        }
    }

    /// Record activity (sets last_activity to now).
    pub fn record_activity(&self) {
        self.last_activity
            .store(chrono::Utc::now().timestamp(), Ordering::Release);
    }
}

#[async_trait]
impl HealthCheck for SimpleHealthCheck {
    fn component_name(&self) -> &str {
        &self.name
    }

    async fn check_health(&self) -> HealthReport {
        HealthReport {
            component: self.name.clone(),
            status: HealthStatus::Healthy,
            message: "OK".to_string(),
            last_activity_at: Some(self.last_activity.load(Ordering::Acquire)),
            duration_ms: None,
        }
    }
}

/// Register a set of default module health checks with the HealthService.
pub async fn register_all_module_checks(service: &super::health_service::HealthService) {
    let module_names = [
        "audit",
        "budget_tracking",
        "cancellation",
        "configuration",
        "dag_engine",
        "enforcement",
        "event_system",
        "execution_engine",
        "failure_classification",
        "planning",
        "repo_engine",
        "risk_gating",
        "state_persistence",
        "template_generation",
        "templates",
        "tools",
    ];

    for name in module_names {
        service
            .register(Arc::new(SimpleHealthCheck::new(name.to_string())))
            .await;
    }
}
