//! Prometheus-style metrics for Rigorix.
//!
//! @canonical .pi/architecture/modules/observability.md#metrics
//!
//! Provides a lightweight metrics registry that outputs Prometheus text format.
//! Supports counters, gauges, and histograms for operational visibility.

#[cfg(not(feature = "portable"))]
pub mod metrics_registry;

#[cfg(not(feature = "portable"))]
pub use metrics_registry::*;
