## Intent

Create a centralized HealthService that aggregates health status from all modules. Add /health, /health/ready, and /health/live endpoints for Kubernetes-style probe support.

## Epic
EPIC-001: Observability Foundation (Milestone #2)

## In Scope
- Create HealthService trait + implementation in observability/health/
- Health checks: budget status, circuit-breaker states, active executions, event bus stats
- Each module provides a HealthCheck implementation
- /health — aggregate status (200 if all healthy, 503 if any degraded)
- /health/ready — readiness probe (all dependencies available)
- /health/live — liveness probe (process is alive)
- Unit tests for HealthService aggregation logic

## Out of Scope
- Tracing infrastructure (ISSUE-001)
- Prometheus metrics (ISSUE-003)
- Individual module health endpoints (ISSUE-004)

## Acceptance Criteria
- [ ] GET /health returns 200 with all modules healthy
- [ ] GET /health/ready returns 200 when all deps available
- [ ] GET /health returns 503 when a module reports Unhealthy
- [ ] Unit tests cover Healthy/Degraded/Unhealthy aggregation
- [ ] All 3 endpoints return structured JSON

## Implementation Notes
- Follow existing health endpoint pattern from dag_engine, state_persistence, and execution_engine
- Each module's health check is registered at startup
- Use tokio::time::timeout to prevent slow health checks from blocking (500ms default)

## Validators Required
- ci, tests, architecture, operations
