# Observability Architecture

> **Status:** Implemented
> **Last verified:** 2026-06-15

## Overview

Centralized observability infrastructure providing structured tracing, health checking, and Prometheus metrics across all 17 modules.

## Components

| Component | File | Purpose |
|-----------|------|---------|
| TracingConfig | `src/observability/tracing_config.rs` | Log level, format, and output configuration |
| init_tracing() | `src/observability/mod.rs` | Initializes tracing subscriber with env-filter |
| SpanPrivacy | `src/observability/span_privacy.rs` | Sensitive field detection (api_key, token, secret) |
| HealthCheck trait | `src/observability/health/health_check.rs` | Component health check interface |
| HealthService | `src/observability/health/health_service.rs` | Aggregates all module health checks |
| MetricsRegistry | `src/observability/metrics/metrics_registry.rs` | Prometheus counters, gauges, histograms |
| SimpleHealthCheck | `src/observability/health/module_health.rs` | Reusable health check for any module |

## Configuration

Tracing is configured via `RIGORIX_LOG` env var (default: `info`). JSON output is default; set `RIGORIX_LOG_FORMAT=pretty` for human-readable output.

## Metrics

| Metric | Type | Description |
|--------|------|-------------|
| rigorix_budget_calls_total | Counter | LLM budget reservations |
| rigorix_retry_attempts_total | Counter | Retry attempts across all nodes |
| rigorix_circuit_breaker_transitions_total | Counter | Circuit breaker state changes |
| rigorix_active_executions | Gauge | Current active DAG executions |
| rigorix_event_bus_subscribers | Gauge | Current event bus subscribers |
| rigorix_budget_remaining_calls | Gauge | Remaining LLM call budget |
| rigorix_execution_latency_seconds | Histogram | Per-node execution latency |
| rigorix_llm_call_duration_seconds | Histogram | LLM API call duration |

## Dependencies

- `tracing` + `tracing-subscriber` + `tracing-appender`
- `prometheus` crate
- All modules consume `#[tracing::instrument]` spans
