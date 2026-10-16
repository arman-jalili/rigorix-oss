# Disaster Recovery Plan: capacity-ledger Module

<!--
Canonical Reference: .pi/architecture/modules/capacity-ledger.md
ADR: ADR-018 (distributed capacity ledger)
Last Updated: 2026-10-15
-->

## Overview

The `capacity-ledger` module is **stateless in the engine**. All durable state
lives in the backing store behind the `CapacityLedger` port:

- `InProcessLedger` keeps state in process memory only — it has no recovery
  concerns beyond restart.
- `HttpLedger` delegates durability to the enterprise ledger service.

This split is why the DR posture depends on which adapter is deployed.

## RTO / RPO Targets

| Deployment | Adapter | RTO | RPO | Rationale |
|------------|---------|-----|-----|-----------|
| Standalone | `InProcessLedger` | < 1 min (process restart) | N/A — in-flight holds are intentionally lost | Ephemeral single-execution accounting; no durable claims to recover |
| Distributed | `HttpLedger` | ≤ 15 min | 0 for committed claims; in-flight leases re-derivable | Committed capacity is durable in the ledger service; uncommitted holds expire |

> For the distributed deployment, the **capacity-ledger-service owns the
> authoritative backup/restore story** (ADR-010). This plan covers the engine's
> adapter and its failover behavior.

## Backup Strategy and Schedule

### Engine side

- No engine-side backup is required or possible: the engine stores no ledger
  state.
- Back up only configuration: `HttpLedgerConfig` (`base_url`, `timeout_ms`,
  `max_response_body_bytes`, token reference). Tokens live in the secret store,
  never in config files or logs.

### Service side (enterprise ledger)

| Item | Schedule | Retention |
|------|----------|-----------|
| Reservation/exposure store | continuous / provider-native | per ADR-010 policy |
| Point-in-time snapshots | hourly | 7 days |
| Pre-deployment snapshot | on every ledger-service deploy | 30 days |

## Restore Procedure

1. **Stop new reservations** at the engine: deploy with `HttpLedger` pointed at a
   read-only/healthy replica, or switch to `InProcessLedger` only if single-
   execution semantics are acceptable for the outage window.
2. **Restore the ledger service** from the most recent snapshot per ADR-010.
3. **Verify exposure** for each declared resource:
   `exposure(resource)` must be `is_consistent()` (`committed + outstanding ≤
   hard_limit`).
4. **Re-declare resources** if specs are not part of the restored store
   (`declare_resource` is idempotent by `ResourceId`).
5. **Resume traffic.** Any reservation whose lease lapsed during the outage is
   void; callers re-reserve with the same idempotency key or a fresh one.

## Failover Plan

| Scenario | Behavior | Action |
|----------|----------|--------|
| Ledger service unreachable | `HttpLedger` maps every ambiguity to `Unavailable`; callers fail closed (never assume capacity) | Fail over to the standby ledger endpoint by reconfiguring `base_url`; retry with backoff |
| Ledger returns non-2xx / oversized / malformed body | `Unavailable` | Treat as outage; do not retry blindly against a corrupt endpoint |
| Region loss | Reservations in the lost region void on lease expiry | Promote the standby store; re-reserve |
| Split brain (two writable stores) | Cross-session atomicity is lost | **Do not** run two writers; fence the old store before promotion |

### Degraded mode

Running `InProcessLedger` as a fallback preserves **single-execution**
atomicity only. It is acceptable only when the operation can tolerate losing
cross-session enforcement for the outage window. Document the downgrade and
restore the distributed adapter afterwards.

## Recovery Validation

After any restore/failover:

1. `declare_resource` for every expected resource.
2. `exposure(resource)` returns `is_consistent() == true`.
3. A reserve/commit/release cycle succeeds end to end.
4. A retried reserve with the same idempotency key returns the same
   `reservation_id`.
5. No `Insufficient` errors that contradict the restored exposure.

## Related

- Runbook: `docs/runbook-capacity-ledger.md`
- ADR-018 — distributed capacity ledger
- ADR-010 — enterprise `capacity-ledger-service`
