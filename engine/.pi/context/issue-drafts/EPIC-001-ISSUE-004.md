---
guardian_issue:
  id: EPIC-001-ISSUE-004
  title: "Health endpoints for remaining 13 modules"
  epic: "Observability Foundation"
  epic_id: EPIC-001
  status: planned
  priority: high
  created_at: "2026-06-15"

  intent: |
    Add health endpoints to the 13 modules that currently lack them. Only
    dag_engine, state_persistence, and execution_engine have HEALTH_PATH
    constants and HealthResponse structs. Add the same pattern to: audit,
    budget_tracking, cancellation, configuration, enforcement, event_system,
    failure_classification, planning, repo_engine, risk_gating,
    template_generation, templates, tools.

  dependencies:
    - name: "EPIC-001-ISSUE-002"
      type: internal
      note: "Module health endpoints should register with the centralized HealthService"

  in_scope:
    - Add HEALTH_PATH constant to each of the 13 modules
    - Add HealthResponse struct to each module's interfaces/http/mod.rs
    - Each module reports: status (up/down), last_activity_at, key_metric
    - Register each module's health check with centralized HealthService (from ISSUE-002)
    - Unit tests for each module's health endpoint

  out_of_scope:
    - Centralized HealthService (ISSUE-002)
    - Metrics (ISSUE-003)

  affected_layers:
    infrastructure:
      - "Modified: 13 modules — add interfaces/http/ with health endpoint"
    api:
      - "Modified: 13 modules — register health check in startup"

  acceptance_criteria:
    - "Each of the 13 modules returns 200 on GET /api/v1/{module}/health"
    - "HealthResponse contains status, last_activity_at, and key_metric"
    - "Each module contributes to centralized HealthService health report"
    - "All 16 modules have consistent HEALTH_PATH and HealthResponse format"

  validators:
    - ci
    - tests
    - architecture
    - operations

  implementation_notes: |
    - Follow the exact pattern from dag_engine/interfaces/http/mod.rs:
      HEALTH_PATH constant, HealthResponse struct, handler function
    - Each module's key_metric differs:
      - audit: last_send_time
      - budget_tracking: remaining_calls
      - cancellation: active_tokens
      - configuration: last_reload_time
      - enforcement: active_warnings
      - event_system: published_count
      - failure_classification: classified_count
      - planning: last_plan_time
      - repo_engine: indexed_symbols
      - risk_gating: pending_gates
      - template_generation: generated_count
      - templates: registered_count
      - tools: executed_tool_count

  file_changes:
    - "modify: engine/src/audit/interfaces/http/mod.rs"
    - "modify: engine/src/budget_tracking/interfaces/http/mod.rs"
    - "modify: engine/src/cancellation/interfaces/http/mod.rs"
    - "modify: engine/src/configuration/interfaces/http/mod.rs"
    - "modify: engine/src/enforcement/interfaces/http/mod.rs"
    - "modify: engine/src/event_system/interfaces/http/mod.rs"
    - "modify: engine/src/failure_classification/interfaces/http/mod.rs"
    - "modify: engine/src/planning/interfaces/http/mod.rs"
    - "modify: engine/src/repo_engine/interfaces/http/mod.rs"
    - "modify: engine/src/risk_gating/interfaces/http/mod.rs"
    - "modify: engine/src/template_generation/interfaces/http/mod.rs"
    - "modify: engine/src/templates/interfaces/http/mod.rs"
    - "modify: engine/src/tools/interfaces/http/mod.rs"
---

# EPIC-001-ISSUE-004: Health endpoints for remaining 13 modules

## Intent

Add consistent health endpoints to all 13 modules that currently lack them.

## Dependencies

```
ISSUE-002 (HealthService) → ISSUE-004 (module health endpoints)
```

## In Scope

- HEALTH_PATH + HealthResponse for 13 modules
- Module-specific key metrics per health response
- Registration with centralized HealthService

## Acceptance Criteria

| # | Criterion | Validator |
|---|-----------|-----------|
| 1 | All 13 modules return 200 on `/health` | Tests |
| 2 | Consistent HealthResponse format across all modules | Architecture |
| 3 | Module-specific key_metric present in each response | Tests |
| 4 | Registered with centralized HealthService | Integration |
