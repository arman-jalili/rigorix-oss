# Runbook: capacity-ledger Module

<!--
Canonical Reference: .pi/architecture/modules/capacity-ledger.md
ADR: ADR-018 (distributed capacity ledger), ADR-008 (RAII reservation pattern)
Last Updated: 2026-10-15
-->

## Overview

The `capacity-ledger` module owns the **model and the port** for contested,
limited capacity shared across independent executions: resources, reservations,
leases, grants, and aggregate exposure. It owns no storage; the engine has no
database dependency. Storage is an adapter.

**Guarantee ceiling (read this first):**

- `InProcessLedger` provides **single-execution atomicity only** (one mutex per
  process).
- Only `HttpLedger` against a real contested store provides **cross-session
  atomicity**.

No deployment may claim cross-session guarantees while running the in-process
adapter.

## Components

| Component | Layer | Role |
|-----------|-------|------|
| `ResourceSpec` / `ResourceId` / `Window` | domain | Operator-declared capacity and aggregation window |
| `Reservation` / `Lease` / `ReservationState` | domain | One held claim, exactly one terminal transition |
| `Grant` | domain | Opaque proof-of-reservation; not caller-constructible |
| `Exposure` | domain | `committed` and `outstanding`, kept distinct |
| `IdempotencyKey` | domain | Retry identity (`for_step` is deterministic) |
| `LedgerError` | domain | `Insufficient \| Contended \| Unavailable \| LeaseExpired` |
| `CapacityLedger` | interfaces | The port (invariants I1–I6) |
| `CapacityLedgerServiceImpl` | application | reserve / commit / release / exposure use cases |
| `ReservationGuardImpl` | application | RAII: commit on success, release on drop |
| `InProcessLedger` | infrastructure | Default adapter, single-execution atomicity |
| `HttpLedger` | infrastructure | Opt-in adapter, fail-closed transport |

## Startup Sequence

1. Declare every resource before reserving against it:
   `CapacityLedger::declare_resource(ResourceSpec)`.
   Reserving an undeclared resource fails closed with `Unavailable`.
2. Construct the adapter:
   - standalone: `InProcessLedger::new()` (or `with_lease_ttl`)
   - distributed: `HttpLedger::new(HttpLedgerConfig::new(base_url, timeout_ms,
     max_body_bytes, auth_token))`
3. Wrap the port in `CapacityLedgerServiceImpl::new(Arc<dyn CapacityLedger>)`.
4. Reserve through the service and hold the returned
   `ReservationGuard` for the step's lifetime.

### Dependencies

| Dependency | Required? | Failure behavior |
|------------|-----------|------------------|
| Tokio runtime | yes (async port) | drop-release logs a warning and relies on lease expiry |
| Enterprise ledger service | only for `HttpLedger` | every ambiguity → `Unavailable` (fail closed) |
| Clock | `Utc::now` (store clock) | expiry is decided by the ledger, never the caller |

## Graceful Shutdown

There is no long-lived worker to drain. Capacity safety on shutdown comes from
the RAII guard and the lease:

1. Dropping an uncommitted `ReservationGuard` dispatches a release onto the
   runtime it was created in.
2. If the process dies before the release runs, the reservation is reclaimed
   when its lease lapses; no manual cleanup is required.
3. On a controlled shutdown, commit or drop every outstanding guard before the
   runtime is torn down so releases are dispatched, not left to expiry.

## Common Failure Modes and Recovery

| Symptom | `LedgerError` | Meaning | Recovery |
|---------|---------------|---------|----------|
| `insufficient capacity …` | `Insufficient` | Resource is healthy but full | Wait for commits/leases to clear, or raise `hard_limit` with `declare_resource` (rejected if below current exposure) |
| `resource … is contended` | `Contended` | Lost a race or replayed a resolved key | Retry with the same idempotency key; do **not** mint a new key |
| `capacity ledger is unavailable` | `Unavailable` | Outage, timeout, non-2xx, malformed/oversized body, undeclared resource, unknown reservation | Fail closed; retry with backoff. Check `HttpLedger` config (timeout, body bound, base URL, token) |
| `lease expired …` | `LeaseExpired` | Commit attempted after the lease lapsed | Re-reserve; the old claim is void |
| Capacity never released after a crash | — | Guard dropped outside a runtime | No action: lease expiry reclaims it |

### Diagnosis

- `exposure(resource)` reports `committed` and `outstanding` separately. A large
  `outstanding` that never falls means leases are not being committed/released —
  check for guards held across long steps.
- `is_retriable()` distinguishes transient (`Contended`, `Unavailable`) from
  terminal (`Insufficient`, `LeaseExpired`) failures.
- `is_consistent()` on an `Exposure` is false if `committed + outstanding`
  exceeds `hard_limit` — a contract violation worth escalating.

## Configuration Reference

### `InProcessLedger`

| Setting | Default | Notes |
|---------|---------|-------|
| lease TTL | 60s | `InProcessLedger::with_lease_ttl(Duration)`; sub-second values are supported |

### `HttpLedgerConfig`

| Setting | Required | Notes |
|---------|----------|-------|
| `base_url` | yes | Ledger service base URL |
| `timeout_ms` | yes | Per-request deadline |
| `max_response_body_bytes` | yes | Responses larger than this are rejected (`Unavailable`) |
| `auth_token` | no | Sent as `Authorization: Bearer`; marked sensitive and redacted in `Debug` |

### Transport discipline (fixed)

No redirects, explicit deadline, bounded body, credentials never logged.

## Health / Observability

- The ledger has no server of its own; `InProcessLedger` is in-process and
  `HttpLedger` health is the enterprise service's `/health`.
- Use `exposure(resource)` as the readiness signal: a successful call means the
  adapter is reachable and consistent.
- All operations are instrumented with `tracing`; correlate by `reservation_id`
  and `resource`.

## Invariants (must hold)

| ID | Invariant |
|----|-----------|
| I1 | Atomic/linearizable per resource+window |
| I2 | Idempotent — a retried reserve with the same key yields one reservation |
| I3 | Leased — every hold expires and is reclaimed |
| I4 | Fail-closed — ambiguity maps to `Unavailable` |
| I5 | Store clock decides expiry |
| I6 | `committed + outstanding` never exceeds `hard_limit` |
