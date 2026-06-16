# Observability

## Module Status

**Status:** ✅ Implemented — contract freeze complete, proofing scripts active
**Last reviewed:** 2026-06-16
**Source session:** 71e2b81a-a7a1-48ee-ab8f-56284bbec92d
**Issues:** #289 (contract freeze), #291 (proofing), #292 (architecture readiness)

## Description

Provides structured tracing, health checking, and metrics collection. Centralized observability layer consumed by all other modules.

- **Tracing**: JSON logging (production) or human-readable (dev), level control via `RIGORIX_LOG` env var
- **Health**: Health check endpoints for daemon mode (future)
- **Metrics**: Prometheus metrics for deployment monitoring
- **Span Privacy**: Redacts sensitive fields (API keys, secrets) from span data

## Architecture

### Clean Architecture Layers

```
observability/
├── domain/           # ObservabilityCliError, ObservabilityEvent
│   ├── mod.rs
│   ├── error.rs      # ObservabilityCliError enum
│   └── event/        # ObservabilityEvent, HealthStatus schemas
│       └── mod.rs
├── application/      # Service traits, DTO schemas
│   ├── mod.rs
│   ├── service.rs    # TracingInitializer trait (frozen)
│   └── dto/          # InitTracing, HealthCheck, Metrics DTOs
│       └── mod.rs
├── infrastructure/   # Trait implementations, repository interfaces
│   ├── mod.rs
│   ├── observability.rs           # Re-exports TracingInitializer
│   ├── tracing.rs                 # init_tracing(), init_default_tracing()
│   └── repository/                # ObservabilityCliRepository trait
│       └── mod.rs
└── interfaces/       # HTTP API contracts
    ├── mod.rs
    └── http/         # 3 endpoints: health, tracing status, metrics
        └── mod.rs
```

## Components

**CLI-facing (contract freeze):**
| Component | File | Module | Purpose |
|-----------|------|--------|---------|
| TracingInitializer (trait) | `cli/src/observability/application/service.rs` | application | Tracing initialization contract (frozen) |
| init_tracing() | `cli/src/observability/infrastructure/tracing.rs` | infrastructure | Engine tracing init with CLI config |
| ObservableCliError | `cli/src/observability/domain/error.rs` | domain | Typed error enum (frozen) |
| ObservabilityEvent | `cli/src/observability/domain/event/observability.rs` | domain | Event schemas (frozen) |
| ObservabilityCliRepository | `cli/src/observability/infrastructure/repository/mod.rs` | infrastructure | Repository interface (frozen) |

## Domain Events

| Event | Description |
|-------|-------------|
| ObservabilityEvent::TracingInitialized | Tracing was initialized |
| ObservabilityEvent::HealthCheckPerformed | A health check completed |
| ObservabilityEvent::HealthStatusChanged | Overall health status changed |

## Key Files

| File | Purpose |
|------|---------|
| `cli/src/observability/application/service.rs` | TracingInitializer trait |
| `cli/src/observability/application/dto/mod.rs` | InitTracing, HealthCheck, Metrics DTOs |
| `cli/src/observability/domain/error.rs` | ObservabilityCliError |
| `cli/src/observability/infrastructure/repository/mod.rs` | ObservabilityCliRepository |
| `cli/docs/runbook-observability.md` | Runbook |
| `cli/docs/dr-plan-observability.md` | DR plan |

## Related Issues

| Issue | Status |
|-------|--------|
| #289 Contract freeze | ✅ Merged (PR #293) |
| #291 Proofing | ✅ Existing stage 13 |
| #292 Architecture readiness | ✅ In progress |
