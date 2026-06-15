//! Metrics registry using the prometheus crate.
//!
//! @canonical .pi/architecture/modules/observability.md#metrics-registry
//!
//! Provides a centralized MetricsRegistry where all modules register their
//! Prometheus counters, gauges, and histograms.

use prometheus::{
    Counter, CounterVec, Gauge, GaugeVec, Histogram, HistogramVec, Registry,
};

/// Centralized metrics registry for the Rigorix observability system.
///
/// All modules register their metrics here at startup. The registry
/// generates Prometheus text format for the /metrics endpoint.
#[derive(Clone)]
pub struct MetricsRegistry {
    /// The underlying Prometheus registry.
    registry: Registry,
}

impl Default for MetricsRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl MetricsRegistry {
    /// Create a new empty metrics registry.
    pub fn new() -> Self {
        Self {
            registry: Registry::new(),
        }
    }

    /// Register and return a counter.
    pub fn counter(
        &self,
        name: &str,
        help: &str,
    ) -> prometheus::Result<Counter> {
        let counter = Counter::new(name, help)?;
        self.registry.register(Box::new(counter.clone()))?;
        Ok(counter)
    }

    /// Register and return a counter with labels.
    pub fn counter_vec(
        &self,
        name: &str,
        help: &str,
        labels: &[&str],
    ) -> prometheus::Result<CounterVec> {
        let counter = CounterVec::new(prometheus::Opts::new(name, help), labels)?;
        self.registry.register(Box::new(counter.clone()))?;
        Ok(counter)
    }

    /// Register and return a gauge.
    pub fn gauge(
        &self,
        name: &str,
        help: &str,
    ) -> prometheus::Result<Gauge> {
        let gauge = Gauge::new(name, help)?;
        self.registry.register(Box::new(gauge.clone()))?;
        Ok(gauge)
    }

    /// Register and return a gauge with labels.
    pub fn gauge_vec(
        &self,
        name: &str,
        help: &str,
        labels: &[&str],
    ) -> prometheus::Result<GaugeVec> {
        let gauge = GaugeVec::new(prometheus::Opts::new(name, help), labels)?;
        self.registry.register(Box::new(gauge.clone()))?;
        Ok(gauge)
    }

    /// Register and return a histogram.
    pub fn histogram(
        &self,
        name: &str,
        help: &str,
        buckets: Vec<f64>,
    ) -> prometheus::Result<Histogram> {
        let histogram = Histogram::with_opts(
            prometheus::HistogramOpts::new(name, help).buckets(buckets),
        )?;
        self.registry.register(Box::new(histogram.clone()))?;
        Ok(histogram)
    }

    /// Register and return a histogram with labels.
    pub fn histogram_vec(
        &self,
        name: &str,
        help: &str,
        labels: &[&str],
        buckets: Vec<f64>,
    ) -> prometheus::Result<HistogramVec> {
        let histogram = HistogramVec::new(
            prometheus::HistogramOpts::new(name, help).buckets(buckets),
            labels,
        )?;
        self.registry.register(Box::new(histogram.clone()))?;
        Ok(histogram)
    }

    /// Gather all registered metrics and return them in Prometheus text format.
    pub fn gather_text(&self) -> String {
        let metric_families = self.registry.gather();
        prometheus::TextEncoder::new()
            .encode_to_string(&metric_families)
            .unwrap_or_else(|e| format!("# Error encoding metrics: {}", e))
    }

    /// Gather all registered metrics as proto families.
    pub fn gather(&self) -> Vec<prometheus::proto::MetricFamily> {
        self.registry.gather()
    }

    /// Get the underlying Prometheus registry (for advanced use).
    pub fn inner(&self) -> &Registry {
        &self.registry
    }
}

/// Predefined metric names for consistency across modules.
pub mod metric_names {
    // Counters
    pub const BUDGET_CALLS_TOTAL: &str = "rigorix_budget_calls_total";
    pub const RETRY_ATTEMPTS_TOTAL: &str = "rigorix_retry_attempts_total";
    pub const CIRCUIT_BREAKER_TRANSITIONS_TOTAL: &str =
        "rigorix_circuit_breaker_transitions_total";

    // Gauges
    pub const ACTIVE_EXECUTIONS: &str = "rigorix_active_executions";
    pub const EVENT_BUS_SUBSCRIBERS: &str = "rigorix_event_bus_subscribers";
    pub const BUDGET_REMAINING_CALLS: &str = "rigorix_budget_remaining_calls";

    // Histograms
    pub const EXECUTION_LATENCY_SECONDS: &str = "rigorix_execution_latency_seconds";
    pub const LLM_CALL_DURATION_SECONDS: &str = "rigorix_llm_call_duration_seconds";

    /// Default histogram buckets for execution latency (in seconds).
    pub fn default_latency_buckets() -> Vec<f64> {
        vec![
            0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
        ]
    }
}

/// Convenience function to create a default registry with standard metrics.
pub fn create_default_metrics() -> prometheus::Result<MetricsRegistry> {
    let registry = MetricsRegistry::new();

    // Core counters
    registry.counter(
        metric_names::BUDGET_CALLS_TOTAL,
        "Total number of LLM budget reservation calls",
    )?;
    registry.counter(
        metric_names::RETRY_ATTEMPTS_TOTAL,
        "Total number of retry attempts across all nodes",
    )?;
    registry.counter(
        metric_names::CIRCUIT_BREAKER_TRANSITIONS_TOTAL,
        "Total number of circuit breaker state transitions",
    )?;

    // Core gauges
    registry.gauge(
        metric_names::ACTIVE_EXECUTIONS,
        "Current number of active DAG executions",
    )?;
    registry.gauge(
        metric_names::EVENT_BUS_SUBSCRIBERS,
        "Current number of event bus subscribers",
    )?;
    registry.gauge(
        metric_names::BUDGET_REMAINING_CALLS,
        "Remaining LLM call budget",
    )?;

    // Core histograms
    registry.histogram(
        metric_names::EXECUTION_LATENCY_SECONDS,
        "Execution latency per node in seconds",
        metric_names::default_latency_buckets(),
    )?;
    registry.histogram(
        metric_names::LLM_CALL_DURATION_SECONDS,
        "LLM API call duration in seconds",
        metric_names::default_latency_buckets(),
    )?;

    Ok(registry)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_default_registry() {
        let registry = create_default_metrics().unwrap();
        let text = registry.gather_text();
        assert!(text.contains("rigorix_budget_calls_total"));
        assert!(text.contains("rigorix_retry_attempts_total"));
        assert!(text.contains("rigorix_active_executions"));
        assert!(text.contains("rigorix_execution_latency_seconds"));
    }

    #[test]
    fn test_counter_increment() {
        let registry = MetricsRegistry::new();
        let counter = registry
            .counter("test_counter", "Test counter")
            .unwrap();
        counter.inc();
        counter.inc_by(3.0);

        let text = registry.gather_text();
        assert!(text.contains("test_counter 4"));
    }

    #[test]
    fn test_gauge_set() {
        let registry = MetricsRegistry::new();
        let gauge = registry.gauge("test_gauge", "Test gauge").unwrap();
        gauge.set(42.0);

        let text = registry.gather_text();
        assert!(text.contains("test_gauge 42"));
    }

    #[test]
    fn test_duplicate_registration_returns_error() {
        let registry = MetricsRegistry::new();
        registry.counter("dup", "first").unwrap();
        let result = registry.counter("dup", "second");
        assert!(result.is_err());
    }
}
