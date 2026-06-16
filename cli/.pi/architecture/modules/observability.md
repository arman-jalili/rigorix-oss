# Observability

## Module Status

**Status:** Engine contract frozen — CLI uses as library
**Last reviewed:** 2026-06-16
**Source session:** 71e2b81a-a7a1-48ee-ab8f-56284bbec92d

## Description

Provides structured tracing, health checking, and metrics collection. Centralized observability layer consumed by all other modules.

- **Tracing**: JSON logging (production) or human-readable (dev), level control via `RIGORIX_LOG` env var
- **Health**: Health check endpoints for daemon mode (future)
- **Metrics**: Prometheus metrics for deployment monitoring
- **Span Privacy**: Redacts sensitive fields (API keys, secrets) from span data

## Components

**CLI-facing:**
| Component | File (planned) | Purpose |
|-----------|---------------|---------|
| TracingInitializer | `cli/src/tracing.rs` | Initializes engine tracing with CLI-specific config |

**Engine dependencies (frozen contracts):**
| Component | Engine Source | Contract |
|-----------|--------------|----------|
| TracingConfig | `engine/src/observability/tracing_config.rs` | Tracing configuration |
| HealthCheck | `engine/src/observability/health/` | Health check endpoints |
| MetricsCollector | `engine/src/observability/metrics/` | Prometheus metrics |
| SpanPrivacyFilter | `engine/src/observability/span_privacy/` | Sensitive field redaction |

## Domain Events

| Event | Description | Triggered By |
|-------|-------------|-------------|
| (subscribes to all ExecutionEvents) | — | — |

## Ubiquitous Language

| Term | Definition |
|------|-----------|
| TracingConfig | Configuration for structured tracing: log format (json/pretty), level, filters. |

## Dependencies

- Depends on: `engine::observability` (tracing, health, metrics)
- Dependencies: tracing, tracing-subscriber, tracing-appender, prometheus
- Used by: All modules (initialized at startup)
