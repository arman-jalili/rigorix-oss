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
| Component | File | Purpose |
|-----------|------|---------|
| TracingInitializer (trait) | `cli/src/infrastructure/observability.rs` | Interface for initializing tracing and health checks |
| init_tracing() | `cli/src/tracing.rs` | Initializes engine tracing with CLI-specific log level and format |
| init_default_tracing() | `cli/src/tracing.rs` | Initializes tracing with safe defaults (pretty, info) |
| ObservabilityEvent | `cli/src/domain/event/observability.rs` | Event schemas: TracingInitialized, HealthCheckPerformed, HealthStatusChanged |

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

## Key Files

| File | Purpose |
|------|---------|
| `cli/src/infrastructure/observability.rs` | TracingInitializer trait (contract) |
| `cli/src/tracing.rs` | Tracing initialization implementation |
| `cli/src/domain/event/observability.rs` | Observability event schemas |
| `cli/src/domain/event/mod.rs` | CliEvent integration (Observability variant) |
| `cli/.pi/scripts/ci/check_observability_contracts.sh` | Automated contract validation (15 checks) |
| `cli/.pi/scripts/ci/check_observability_coverage.sh` | Coverage threshold enforcement |
| `cli/.pi/scripts/ci/stage_observability_proofing.sh` | CI stage wrapper (stage 13) |
| `engine/src/observability/tracing_config.rs` | Tracing config (level, format) |
| `engine/src/observability/health/` | Health check endpoints and aggregator |
| `engine/src/observability/metrics/` | Prometheus metrics collection |
| `engine/src/observability/span_privacy.rs` | Sensitive field redaction |

## ADRs

| ADR | Title | Status |
|-----|-------|--------|
| ADR-001 | Domain-Driven Design with Bounded Contexts | Accepted |

## Proofing Scripts

| Script | Purpose | Stage |
|--------|---------|-------|
| `check_observability_contracts.sh` | 15 automated checks for observability contracts (trait, events, wiring) | stage 13 — observability_proofing |
| `check_observability_coverage.sh` | Coverage thresholds (2+ tracing tests, 35+ overall) | stage 13 — observability_proofing |
| `stage_observability_proofing.sh` | CI stage wrapper | stage 13 — observability_proofing |
