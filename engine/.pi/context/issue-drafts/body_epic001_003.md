## Intent

Add Prometheus metrics to expose operational data: budget consumption rate, retry frequency, execution latency distribution, circuit-breaker state transitions, and event bus throughput. Create a MetricsRegistry for all modules to register their metrics with.

## Epic
EPIC-001: Observability Foundation (Milestone #2)

## In Scope
- Add prometheus crate to Cargo.toml
- Create MetricsRegistry in observability/metrics/
- Define counters: budget_calls_total, retry_attempts_total, circuit_breaker_transitions_total
- Define gauges: active_executions, event_bus_subscribers, budget_remaining_calls
- Define histograms: execution_latency_seconds, llm_call_duration_seconds
- /metrics endpoint for Prometheus scraping
- Metric naming follows Prometheus conventions
- Access control on /metrics (internal network check or Metrics-Auth header)
- Unit tests for metric registration and increment

## Operations Condition (from validator)
/metrics must expose at minimum: budget consumption rate, retry frequency, execution latency histogram.

## Security Condition (from validator)
/metrics endpoint must have access control (internal network or auth header).

## Out of Scope
- Individual module health endpoints (ISSUE-004)
- Full dashboard/Grafana setup

## Acceptance Criteria
- [ ] GET /metrics returns valid Prometheus text format
- [ ] budget_calls_total increments on each LLM call
- [ ] execution_latency_seconds histogram captures node execution times
- [ ] Access control restricts /metrics (verifiable in test)
- [ ] MetricsRegistry rejects duplicate metric registration

## Implementation Notes
- Use prometheus crate counters, gauges, and histograms
- MetricsRegistry wraps prometheus::Registry with module-scoped sub-registries
- Access control: check for X-Metrics-Auth header or verify internal IP range
- Do NOT expose request content or user data in metrics labels

## Validators Required
- ci, tests, security, operations
