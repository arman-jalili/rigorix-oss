---
guardian_issue:
  id: "ISSUE-READINESS"
  epic: ""action-input""
  component: "Architecture Readiness"
  module: "action-input"
  status: planned
  priority: critical
  dependencies: []

  in_scope:
    - Create runbook (startup, shutdown, recovery procedures)
    - Create DR plan (backup, restore, failover)
    - Add observability (metrics, tracing, structured logging)
    - Add health check endpoints
    - Update architecture documentation
    - Sync canonical references
    - Verify CI enforces all the above

  out_of_scope:
    - New feature work
    - Implementation changes

  affected_layers:
    domain:
      - Architecture documentation updates
    application:
      - Observability hooks
    infrastructure:
      - Health checks, monitoring config
    ci:
      - Verify proofing scripts + validators in CI

  canonical_references:
    - module: ".pi/architecture/modules/action-input.md"

  acceptance_criteria:
    - "Runbook created and reviewed"
    - "DR plan documented"
    - "Observability patterns in place (tracing, metrics, logging)"
    - "Health check endpoint responds"
    - "Architecture docs synced with implementation"
    - "Canonical references verified (validate-canonical.sh passes)"
    - "Proofing scripts integrated in CI and passing"
    - "All validators pass: ci, tests, security, architecture, canonical, operations"

  validators:
    - ci
    - tests
    - security
    - architecture
    - canonical
    - operations

  implementation_notes: |
    The final issue in every epic. Production readiness means: the team can operate it
    (runbook), recover from failure (DR plan), observe it (metrics/tracing/logging),
    and CI will catch regressions (proofing scripts + validators).

  file_changes:
    - "create: docs/runbook-action-input.md"
    - "create: docs/dr-plan-action-input.md"
    - "modify: .pi/architecture/CHANGELOG.md"
    - "modify: .pi/architecture/modules/action-input.md"
---

# Architecture Readiness: action-input

## Intent

Make the action-input module production-ready. This is the final issue in every epic
— it closes the loop between implementation and operability.

## Deliverables

### Runbook
`docs/runbook-action-input.md` covering:
- Startup sequence and dependencies
- Graceful shutdown procedure
- Common failure modes and recovery
- Configuration reference

### DR Plan
`docs/dr-plan-action-input.md` covering:
- Backup strategy and schedule
- Restore procedure
- Failover plan
- RTO/RPO targets

### Observability
- Metrics: key business and technical metrics exposed
- Tracing: distributed tracing context propagated
- Logging: structured logging with correlation IDs
- Health: /health endpoint with dependency checks

### CI Enforcement
Verify that:
- Proofing scripts from the proofing issue are in CI
- All validators (ci, tests, security, architecture, canonical, operations) pass
- A CI pipeline run against this state succeeds

## Acceptance Criteria

| # | Criterion | Validator |
|---|-----------|-----------|
| 1 | Runbook exists | manual review |
| 2 | DR plan exists | manual review |
| 3 | Observability patterns present | validate-operations.sh |
| 4 | Canonical references synced | validate-canonical.sh |
| 5 | CI enforce validators | validate-ci.sh |
| 6 | All proofing scripts pass | run_hardening_stages.sh |
| 7 | Architecture docs updated | validate-architecture.sh |

## Implementation

> **Agent:** Close out the epic properly:
> 1. Write runbook and DR plan docs
> 2. Add observability instrumentation
> 3. Update architecture module docs with final implementation details
> 4. Sync CHANGE LOG
> 5. Verify proofing scripts from the proofing issue pass
> 6. Run full validation suite
> 7. Architecture readiness validator: bash .pi/scripts/validate-architecture-readiness.sh
> 8. Create final MR
