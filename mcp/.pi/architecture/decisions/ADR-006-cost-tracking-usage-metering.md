# ADR-006: Cost Tracking & Usage Metering — Engine-Delegated + Proxy Telemetry

**Status:** Accepted
**Date:** 2026-07-12
**Session:** d19b7a21-8f4c-4b3e-9a1d-5e6f7c8b9a0d

## Context

The MCP Gateway needs to expose enforcement status and cost/budget information to AI assistants. Per the functional requirements:

- `rigorix_check_enforcement` — Returns remaining budget, active limits, circuit breaker status
- `rigorix_audit_summary` — Returns aggregate statistics over a time range (total executions, token usage, failure rates)

The actual budget tracking, token metering, and enforcement logic lives in **rigorix-engine**, not the MCP Gateway. The gateway is a query-only bridge. There is also a potential need to meter enterprise API usage for billing purposes.

The challenge: How does the gateway report cost/budget data that it doesn't own?

## Decision

1. **Execution cost tracking**: Delegated entirely to rigorix-engine. The `EngineFacade` trait includes methods to query:
   - `check_enforcement() -> EnforcementStatus` — current budget, limits, circuit breaker state
   - `execution_cost(execution_id) -> CostBreakdown` — tokens used, tool calls, duration
   - The gateway never performs its own cost calculations

2. **OSS budget tracking**: Not applicable. OSS mode has no budgets — `rigorix_check_enforcement` always returns `{active: false, preset: "unlimited"}`. Budgets are an engine-level concern for organizations that configure them.

3. **Enterprise metering**: The enterprise proxy can include telemetry on `rigorix_enterprise_*` call volume and latency. This is metadata attached to proxy requests, not tracked at the gateway level.

4. **Aggregate audit summaries**: `AuditSummaryHandler` computes aggregates from rigorix-engine audit query results. No local aggregation. Engine returns counts, rates, top failures/templates.

## Consequences

- **Positive**: Zero cost tracking logic in the gateway — simpler, less bug-prone
- **Positive**: The gateway faithfully reports engine state without duplicating business logic
- **Positive**: Enterprises with budget enforcement still get accurate data via engine delegation
- **Negative**: OSS users get minimal enforcement status (always unlimited) — acceptable since OSS has no budgets
- **Negative**: Every enforcement check requires a round-trip to rigorix-engine (acceptable — engine is local, sub-ms queries)

## Alternatives Considered

| Alternative | Rationale for Rejection |
|-------------|------------------------|
| Local budget tracking in gateway | Duplicates engine logic; budget state would diverge between gateway and engine |
| Token metering in gateway via LLM provider SDKs | Gateway is an execution bridge, not an LLM client; token costs are engine-level concern |
| Cached enforcement status to avoid engine calls | Status must be real-time (budgets change on every execution); staleness could allow over-budget execution |
| Separate telemetry pipeline for enterprise | Adds complexity; enterprise API already receives call data via proxy — no separate pipeline needed |

## Affected Modules

- Execution Tools (EngineFacade enforcement queries)
- Audit Tools (AuditSummaryHandler engine delegation)
- Enterprise Proxy (proxy call telemetry)
