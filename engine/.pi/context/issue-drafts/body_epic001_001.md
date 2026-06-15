## Intent

Add the tracing crate ecosystem to the project and instrument every public service method across all 17 modules with #[tracing::instrument]. Wire tracing to both stdout (development) and the EventBus for structured observability.

## Epic
EPIC-001: Observability Foundation (Milestone #2)

## In Scope
- Add tracing, tracing-subscriber, tracing-appender to Cargo.toml
- Create TracingConfig in Configuration module (level, format, output)
- Add #[tracing::instrument] to every public service method across all 17 modules
- Add spans for: LLM API calls, retry decisions, DAG node transitions, budget reserve/commit, cancellation propagation
- Wire tracing to EventBus for dual emission (structured logs + events)
- Implement SpanPrivacy filter: redact fields matching api_key, token, secret, password
- Unit tests for SpanPrivacy filter

## Security Condition (from validator)
SpanPrivacy filter must redact API keys, tokens, secrets, and passwords from tracing output.

## Out of Scope
- Health endpoints (ISSUE-002)
- Metrics endpoints (ISSUE-003)
- Individual module health endpoints (ISSUE-004)

## Acceptance Criteria
- [ ] cargo build on a fresh checkout — no errors
- [ ] All 889 existing tests pass (cargo test)
- [ ] Zero compiler warnings
- [ ] SpanPrivacy filter correctly redacts API keys from tracing output (unit test)
- [ ] #[instrument] on all public service methods (grep verification in CI)

## Implementation Notes
- Instrument module by module in alphabetical order
- SpanPrivacy filter: match field names against a regex set; if matched, replace value with "[REDACTED]"
- Dual emission: subscribe EventBus to tracing events via tracing-subscriber's Layer trait
- Do NOT instrument test helpers or #[cfg(test)] modules

## Files Changed
- modify: engine/Cargo.toml (add tracing, tracing-subscriber, tracing-appender)
- modify: engine/src/lib.rs (add tracing initialization)
- create: engine/src/observability/tracing_config.rs, span_privacy.rs, event_bus_layer.rs
- modify: engine/src/*/application/service*.rs (add #[instrument]) — all 17 modules

## Validators Required
- ci, tests, security, architecture, canonical
