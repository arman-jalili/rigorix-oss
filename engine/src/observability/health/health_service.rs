//! Centralized HealthService for aggregating component health.
//!
//! @canonical .pi/architecture/modules/observability.md#health-service

use super::health_check::{HealthCheck, HealthReport, HealthStatus};
use serde::Serialize;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

/// The aggregate health status across all registered components.
#[derive(Debug, Clone, Serialize)]
pub struct AggregateHealth {
    /// Overall status: Healthy if all components healthy, Degraded if any
    /// degraded, Unhealthy if any unhealthy.
    pub status: HealthStatus,
    /// Per-component health reports.
    pub components: Vec<HealthReport>,
    /// Number of healthy components.
    pub healthy_count: usize,
    /// Number of degraded components.
    pub degraded_count: usize,
    /// Number of unhealthy components.
    pub unhealthy_count: usize,
}

/// Centralized service for aggregating component health checks.
pub struct HealthService {
    /// Registered health checks.
    checks: RwLock<Vec<Arc<dyn HealthCheck>>>,
    /// Default timeout for health checks.
    default_timeout: Duration,
}

impl HealthService {
    /// Create a new HealthService with the given default timeout.
    pub fn new(default_timeout: Duration) -> Self {
        Self {
            checks: RwLock::new(Vec::new()),
            default_timeout,
        }
    }

    /// Register a health check component.
    pub async fn register(&self, check: Arc<dyn HealthCheck>) {
        let mut checks = self.checks.write().await;
        checks.push(check);
    }

    /// Run all registered health checks and aggregate results.
    pub async fn check_all(&self) -> AggregateHealth {
        let checks = self.checks.read().await;
        let mut reports = Vec::with_capacity(checks.len());

        for check in checks.iter() {
            let report = check.check_health_with_timeout(self.default_timeout).await;
            reports.push(report);
        }

        let healthy_count = reports
            .iter()
            .filter(|r| r.status == HealthStatus::Healthy)
            .count();
        let degraded_count = reports
            .iter()
            .filter(|r| r.status == HealthStatus::Degraded)
            .count();
        let unhealthy_count = reports
            .iter()
            .filter(|r| r.status == HealthStatus::Unhealthy)
            .count();

        let status = if unhealthy_count > 0 {
            HealthStatus::Unhealthy
        } else if degraded_count > 0 {
            HealthStatus::Degraded
        } else {
            HealthStatus::Healthy
        };

        AggregateHealth {
            status,
            components: reports,
            healthy_count,
            degraded_count,
            unhealthy_count,
        }
    }

    /// Quick liveness check — is the process alive?
    pub async fn is_alive(&self) -> bool {
        true
    }

    /// Readiness check — are all required components healthy?
    pub async fn is_ready(&self) -> bool {
        let health = self.check_all().await;
        unhealthy_count(&health) == 0
    }
}

fn unhealthy_count(health: &AggregateHealth) -> usize {
    health
        .components
        .iter()
        .filter(|r| r.status == HealthStatus::Unhealthy)
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::observability::health::health_check::HealthCheck;

    struct MockHealthCheck {
        name: String,
        status: HealthStatus,
    }

    #[async_trait::async_trait]
    impl HealthCheck for MockHealthCheck {
        fn component_name(&self) -> &str {
            &self.name
        }

        async fn check_health(&self) -> HealthReport {
            match self.status {
                HealthStatus::Healthy => HealthReport::healthy(&self.name),
                HealthStatus::Degraded => HealthReport::degraded(&self.name, "degraded"),
                HealthStatus::Unhealthy => HealthReport::unhealthy(&self.name, "unhealthy"),
            }
        }
    }

    #[tokio::test]
    async fn test_all_healthy() {
        let service = HealthService::new(Duration::from_secs(1));
        service
            .register(Arc::new(MockHealthCheck {
                name: "db".to_string(),
                status: HealthStatus::Healthy,
            }))
            .await;
        service
            .register(Arc::new(MockHealthCheck {
                name: "cache".to_string(),
                status: HealthStatus::Healthy,
            }))
            .await;

        let health = service.check_all().await;
        assert_eq!(health.status, HealthStatus::Healthy);
        assert_eq!(health.healthy_count, 2);
    }

    #[tokio::test]
    async fn test_some_unhealthy() {
        let service = HealthService::new(Duration::from_secs(1));
        service
            .register(Arc::new(MockHealthCheck {
                name: "db".to_string(),
                status: HealthStatus::Healthy,
            }))
            .await;
        service
            .register(Arc::new(MockHealthCheck {
                name: "api".to_string(),
                status: HealthStatus::Unhealthy,
            }))
            .await;

        let health = service.check_all().await;
        assert_eq!(health.status, HealthStatus::Unhealthy);
        assert_eq!(health.healthy_count, 1);
        assert_eq!(health.unhealthy_count, 1);
    }

    #[tokio::test]
    async fn test_degraded() {
        let service = HealthService::new(Duration::from_secs(1));
        service
            .register(Arc::new(MockHealthCheck {
                name: "db".to_string(),
                status: HealthStatus::Healthy,
            }))
            .await;
        service
            .register(Arc::new(MockHealthCheck {
                name: "cache".to_string(),
                status: HealthStatus::Degraded,
            }))
            .await;

        let health = service.check_all().await;
        assert_eq!(health.status, HealthStatus::Degraded);
        assert_eq!(health.degraded_count, 1);
    }

    #[tokio::test]
    async fn test_timeout_for_slow_check() {
        struct SlowCheck;

        #[async_trait::async_trait]
        impl HealthCheck for SlowCheck {
            fn component_name(&self) -> &str {
                "slow"
            }

            async fn check_health(&self) -> HealthReport {
                tokio::time::sleep(Duration::from_millis(500)).await;
                HealthReport::healthy("slow")
            }
        }

        let service = HealthService::new(Duration::from_millis(50));
        service.register(Arc::new(SlowCheck)).await;

        let health = service.check_all().await;
        // The slow check should time out and report unhealthy
        assert_eq!(health.status, HealthStatus::Unhealthy);
    }
}
