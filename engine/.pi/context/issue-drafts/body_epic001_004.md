## Intent

Add health endpoints to the 13 modules that currently lack them. Only dag_engine, state_persistence, and execution_engine have HEALTH_PATH constants and HealthResponse structs. Add the same pattern to all remaining modules.

## Epic
EPIC-001: Observability Foundation (Milestone #2)

## In Scope
- Add HEALTH_PATH constant to each of the 13 modules
- Add HealthResponse struct to each module's interfaces/http/mod.rs
- Each module reports: status (up/down), last_activity_at, key_metric
- Register each module's health check with centralized HealthService (from ISSUE-002)
- Unit tests for each module's health endpoint

## Out of Scope
- Centralized HealthService (ISSUE-002)
- Metrics (ISSUE-003)

## Acceptance Criteria
- [ ] Each of the 13 modules returns 200 on GET /api/v1/{module}/health
- [ ] HealthResponse contains status, last_activity_at, and key_metric
- [ ] Each module contributes to centralized HealthService health report
- [ ] All 16 modules have consistent HEALTH_PATH and HealthResponse format

## Implementation Notes
- Follow the exact pattern from dag_engine/interfaces/http/mod.rs
- Each module's key_metric differs (e.g., audit: last_send_time, budget_tracking: remaining_calls, event_system: published_count)

## Files Changed (13 modules)
- modify: src/audit/interfaces/http/mod.rs
- modify: src/budget_tracking/interfaces/http/mod.rs
- modify: src/cancellation/interfaces/http/mod.rs
- modify: src/configuration/interfaces/http/mod.rs
- modify: src/enforcement/interfaces/http/mod.rs
- modify: src/event_system/interfaces/http/mod.rs
- modify: src/failure_classification/interfaces/http/mod.rs
- modify: src/planning/interfaces/http/mod.rs
- modify: src/repo_engine/interfaces/http/mod.rs
- modify: src/risk_gating/interfaces/http/mod.rs
- modify: src/template_generation/interfaces/http/mod.rs
- modify: src/templates/interfaces/http/mod.rs
- modify: src/tools/interfaces/http/mod.rs

## Validators Required
- ci, tests, architecture, operations
