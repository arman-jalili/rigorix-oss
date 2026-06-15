---
guardian_issue:
  id: EPIC-001-ISSUE-003
  title: "Prometheus /metrics endpoints"
  epic: "Observability Foundation"
  epic_id: EPIC-001
  status: planned
  priority: critical
  created_at: "2026-06-15"

  intent: |
    Add Prometheus metrics to expose operational data: budget consumption rate,
    retry frequency, execution latency distribution, circuit-breaker state
    transitions, and event bus throughput. Create a MetricsRegistry for all
    modules to register their metrics with.

  dependencies:
    - name: "EPIC-001-ISSUE-001"
      type: internal
      note: "Metrics can use tracing spans for automatic latency histograms"

  in_scope:
    - Add `prometheus` crate to Cargo.toml
    - Create `MetricsRegistry` in `observability/metrics/`
    - Define counters: budget_calls_total, retry_attempts_total, circuit_breaker_transitions_total
    - Define gauges: active_executions, event_bus_subscribers, budget_remaining_calls
    - Define histograms: execution_latency_seconds, llm_call_duration_seconds
    - `/metrics` endpoint for Prometheus scraping
    - Metric naming follows Prometheus conventions (snake_case, _total for counters)
    - Access control on `/metrics` (internal network check or Metrics-Auth header)
    - Unit tests for metric registration and increment

  out_of_scope:
    - Individual module health endpoints (ISSUE-004)
    - Full dashboard/Grafana setup

  affected_layers:
    domain:
      - "New: MetricKey, MetricValue types"
    application:
      - "New: MetricsRegistryService"
      - "New: MetricsCollector trait"
    infrastructure:
      - "New: PrometheusMetricsCollector"
    api:
      - "New: /metrics endpoint"

  acceptance_criteria:
    - "GET /metrics returns valid Prometheus text format"
    - "budget_calls_total increments on each LLM call"
    - "execution_latency_seconds histogram captures node execution times"
    - "Access control restricts /metrics (verifiable in test)"
    - "MetricsRegistry rejects duplicate metric registration"

  validators:
    - ci
    - tests
    - security
    - operations

  implementation_notes: |
    - Use `prometheus` crate counters, gauges, and histograms
    - MetricsRegistry wraps prometheus::Registry with module-scoped sub-registries
    - Each module registers its metrics at startup via MetricsRegistry::register_module()
    - Access control: check for X-Metrics-Auth header or verify internal IP range
    - Prometheus text format is the default output format
    - Do NOT expose request content or user data in metrics labels

  file_changes:
    - "modify: engine/Cargo.toml (add prometheus crate)"
    - "create: engine/src/observability/metrics/metrics_registry.rs"
    - "create: engine/src/observability/metrics/metrics_endpoint.rs"
    - "create: engine/src/observability/metrics/metric_collectors.rs"
    - "create: engine/src/observability/metrics/mod.rs"
    - "modify: engine/src/observability/mod.rs"
---

# EPIC-001-ISSUE-003: Prometheus /metrics endpoints

## Intent

Add Prometheus metrics for operational visibility into budget consumption, retry rates, execution latency, and circuit-breaker state.

## Dependencies

```
ISSUE-001 (tracing) → ISSUE-003 (metrics) — tracing spans feed latency histograms
```

## In Scope

- `prometheus` crate + `MetricsRegistry`
- Counters, gauges, histograms for key operational metrics
- `/metrics` endpoint in Prometheus text format
- Access control on `/metrics`

## Acceptance Criteria

| # | Criterion | Validator |
|---|-----------|-----------|
| 1 | `/metrics` returns valid Prometheus text format | Tests |
| 2 | `budget_calls_total` increments on each LLM call | Tests |
| 3 | `execution_latency_seconds` captures execution times | Tests |
| 4 | Access control prevents unauthorized access | Security |
| 5 | No duplicate metric registration | Tests |
