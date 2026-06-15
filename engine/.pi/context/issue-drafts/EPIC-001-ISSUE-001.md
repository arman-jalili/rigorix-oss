---
guardian_issue:
  id: EPIC-001-ISSUE-001
  title: "Add tracing crate + instrument all services"
  epic: "Observability Foundation"
  epic_id: EPIC-001
  status: planned
  priority: critical
  created_at: "2026-06-15"

  intent: |
    Add the tracing crate ecosystem to the project and instrument every public
    service method across all 17 modules with #[tracing::instrument]. Wire
    tracing to both stdout (development) and the EventBus for structured
    observability.

  dependencies:
    - name: none
      type: internal
      note: "Root issue — no internal dependencies"

  in_scope:
    - Add `tracing`, `tracing-subscriber`, `tracing-appender` to Cargo.toml
    - Create `TracingConfig` in Configuration module (level, format, output)
    - Add `#[tracing::instrument]` to every public service method across all 17 modules
    - Add spans for: LLM API calls, retry decisions, DAG node transitions, budget reserve/commit, cancellation propagation
    - Wire tracing to EventBus for dual emission (structured logs + events)
    - Implement SpanPrivacy filter: redact fields matching api_key, token, secret, password
    - Unit tests for SpanPrivacy filter
    - Verify zero `unwrap()` in tracing code paths

  out_of_scope:
    - Health endpoints (EPIC-001-ISSUE-002)
    - Metrics endpoints (EPIC-001-ISSUE-003)
    - Individual module health endpoints (EPIC-001-ISSUE-004)

  affected_layers:
    domain:
      - "New: TracingConfig entity"
    application:
      - "New: TracingInitializer service"
    infrastructure:
      - "Modified: All 17 module application layers — add #[instrument]"

  canonical_references:
    - module: ".pi/architecture/modules/configuration.md#observability"
    - module: ".pi/architecture/modules/event-system.md#observability"
    - pattern: ".pi/context/patterns.md#tracing--logging"
    - adr:
        - "ADR-005: Event Bus Persistence (tracing emits to EventBus)"

  acceptance_criteria:
    - "cargo build on a fresh checkout — no errors"
    - "All 889 existing tests pass (cargo test)"
    - "Zero compiler warnings"
    - "SpanPrivacy filter correctly redacts API keys from tracing output (unit test)"
    - "#[instrument] on all public service methods (grep verification in CI)"

  validators:
    - ci
    - tests
    - security
    - architecture
    - canonical

  implementation_notes: |
    - Approach: Add tracing deps to Cargo.toml, create TracingConfig, then
      instrument module by module in alphabetical order (audit → budget_tracking
      → cancellation → configuration → dag_engine → enforcement → event_system
      → execution_engine → failure_classification → planning → repo_engine →
      risk_gating → state_persistence → template_generation → templates → tools)
    - SpanPrivacy filter: match field names against a regex set; if matched,
      replace value with "[REDACTED]"
    - Dual emission: subscribe EventBus to tracing events so every important
      state change appears in both structured logs and the EventBus
    - Use tracing-subscriber's `Layer` trait for EventBus integration
    - Do NOT instrument test helpers or #[cfg(test)] modules
    - Key pattern from patterns.md: `#[instrument(skip(non_debug_param), fields(user_id = %user_id))]`

  file_changes:
    - "modify: engine/Cargo.toml (add tracing, tracing-subscriber, tracing-appender)"
    - "modify: engine/src/lib.rs (add tracing initialization)"
    - "create: engine/src/observability/tracing_config.rs"
    - "create: engine/src/observability/span_privacy.rs"
    - "create: engine/src/observability/event_bus_layer.rs"
    - "modify: engine/src/observability/mod.rs"
    - "modify: engine/src/audit/**/service*.rs"
    - "modify: engine/src/budget_tracking/**/service*.rs"
    - "modify: engine/src/cancellation/**/service*.rs"
    - "modify: engine/src/configuration/**/service*.rs"
    - "modify: engine/src/dag_engine/**/service*.rs"
    - "modify: engine/src/enforcement/**/service*.rs"
    - "modify: engine/src/event_system/**/service*.rs"
    - "modify: engine/src/execution_engine/**/service*.rs"
    - "modify: engine/src/failure_classification/**/service*.rs"
    - "modify: engine/src/planning/**/service*.rs"
    - "modify: engine/src/repo_engine/**/service*.rs"
    - "modify: engine/src/risk_gating/**/service*.rs"
    - "modify: engine/src/state_persistence/**/service*.rs"
    - "modify: engine/src/template_generation/**/service*.rs"
    - "modify: engine/src/tools/**/service*.rs"

  pipeline_steps:
    - implement
    - validate
    - create-mr
    - merge
---

# EPIC-001-ISSUE-001: Add tracing crate + instrument all services

## Intent

Add structured tracing to the project and instrument every public service method. Critical for production observability (C-01).

## Dependencies

No internal dependencies. Root issue.

## In Scope

- `tracing` / `tracing-subscriber` / `tracing-appender` in Cargo.toml
- `TracingConfig` in Configuration module
- `#[tracing::instrument]` on all ~25 public service methods across 17 modules
- Spans for key operations: LLM API calls, retry decisions, DAG transitions, budget ops, cancellation
- Dual emission: stdout + EventBus
- `SpanPrivacy` filter for secrets redaction

## Out of Scope

- Health endpoints (ISSUE-002)
- Metrics endpoints (ISSUE-003)
- Individual module health (ISSUE-004)

## Acceptance Criteria

| # | Criterion | Validator |
|---|-----------|-----------|
| 1 | `cargo build` passes on fresh checkout | CI |
| 2 | All 889+ tests pass | Tests |
| 3 | Zero compiler warnings | CI |
| 4 | SpanPrivacy redacts API keys (verified by unit test) | Security |
| 5 | All public service methods have `#[instrument]` (grep check) | Operations |

## Implementation Steps

1. Add deps to Cargo.toml
2. Create `observability/` module with TracingConfig, SpanPrivacy, EventBusLayer
3. Instrument services module by module (alphabetical)
4. Add tracing initialization to `lib.rs`
5. Write SpanPrivacy filter unit tests
6. Run full test suite
7. Run `cargo build` with zero warnings

## Notes

- Use `tracing-subscriber::Layer` trait for EventBus integration
- SpanPrivacy: regex-based field redaction for `api_key`, `token`, `secret`, `password`
- Do NOT instrument `#[cfg(test)]` code
