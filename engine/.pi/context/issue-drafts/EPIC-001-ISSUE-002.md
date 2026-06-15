---
guardian_issue:
  id: EPIC-001-ISSUE-002
  title: "Centralized HealthService"
  epic: "Observability Foundation"
  epic_id: EPIC-001
  status: planned
  priority: critical
  created_at: "2026-06-15"

  intent: |
    Create a centralized HealthService that aggregates health status from all
    modules. Add /health, /health/ready, and /health/live endpoints for
    Kubernetes-style probe support.

  dependencies:
    - name: "EPIC-001-ISSUE-001"
      type: internal
      note: "Tracing infrastructure should be available for health check logging"

  in_scope:
    - Create `HealthService` trait + implementation in `observability/health/`
    - Health checks: budget status, circuit-breaker states, active executions, event bus stats
    - Each module provides a `HealthCheck` implementation
    - `/health` — aggregate status (200 if all healthy, 503 if any degraded)
    - `/health/ready` — readiness probe (all dependencies available)
    - `/health/live` — liveness probe (process is alive)
    - Unit tests for HealthService aggregation logic

  out_of_scope:
    - Tracing infrastructure (ISSUE-001)
    - Prometheus metrics (ISSUE-003)
    - Individual module health endpoints (ISSUE-004)

  affected_layers:
    domain:
      - "New: HealthStatus enum (Healthy/Degraded/Unhealthy)"
      - "New: HealthCheck trait"
      - "New: HealthReport struct"
    application:
      - "New: HealthServiceImpl"
      - "New: HealthServiceFactory"
    infrastructure:
      - "New: Module registry for health check registration"
    api:
      - "New: /health, /health/ready, /health/live endpoints"

  acceptance_criteria:
    - "GET /health returns 200 with all modules healthy"
    - "GET /health/ready returns 200 when all deps available"
    - "GET /health returns 503 when a module reports Unhealthy"
    - "Unit tests cover Healthy/Degraded/Unhealthy aggregation"
    - "All 3 endpoints return structured JSON"

  validators:
    - ci
    - tests
    - architecture
    - operations

  implementation_notes: |
    - Follow existing health endpoint pattern from dag_engine, state_persistence,
      and execution_engine but centralized
    - Each module's health check is registered at startup
    - HealthService runs checks on-demand (not cached) for freshness
    - Use tokio::time::timeout to prevent slow health checks from blocking (500ms default)

  file_changes:
    - "create: engine/src/observability/health/health_check.rs"
    - "create: engine/src/observability/health/health_service.rs"
    - "create: engine/src/observability/health/health_endpoints.rs"
    - "create: engine/src/observability/health/mod.rs"
    - "modify: engine/src/observability/mod.rs"
---

# EPIC-001-ISSUE-002: Centralized HealthService

## Intent

Create a centralized HealthService and /health endpoints for production lifecycle management.

## Dependencies

```
EPIC-001-ISSUE-001 (tracing) → EPIC-001-ISSUE-002 (HealthService) ← ISSUE-004 (module health)
```

## In Scope

- `HealthCheck` trait + `HealthService` implementation
- `/health` / `/health/ready` / `/health/live` endpoints
- Module health check registry
- Aggregation logic: Healthy/Degraded/Unhealthy

## Acceptance Criteria

| # | Criterion | Validator |
|---|-----------|-----------|
| 1 | `/health` returns 200 when all modules healthy | Tests |
| 2 | `/health` returns 503 when any module Unhealthy | Tests |
| 3 | `/health/ready` passes when deps available | Tests |
| 4 | All endpoints return structured JSON | Tests |
| 5 | Timeout on slow health checks (500ms) | Operations |
