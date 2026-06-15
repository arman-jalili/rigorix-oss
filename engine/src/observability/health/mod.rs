//! Centralized health checking for Rigorix.
//!
//! @canonical .pi/architecture/modules/observability.md#health
//!
//! Provides a centralized `HealthService` that aggregates health status
//! from all modules. Supports Kubernetes-style /health, /health/ready,
//! and /health/live probes.

pub mod health_check;
pub mod health_service;

pub mod module_health;

pub use health_check::*;
pub use health_service::*;
pub use module_health::*;
