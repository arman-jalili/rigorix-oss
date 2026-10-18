---
guardian_issue:
  id: "ISSUE-CAPACITY-IDEMPOTENCY-PAYLOAD-MISMATCH"
  title: "capacity_ledger: reusing an idempotency key with a different amount silently returns the original reservation"
  epic: "capacity ledger"
  component: "CapacityLedger"
  module: "capacity-ledger"
  status: planned
  priority: high
  dependencies:
    - "ADR-018 distributed capacity ledger"
    - "ISSUE-CAPACITY-AMBIGUOUS-OUTCOME (expiry also drops the key — same key lifecycle)"

  intent: |
    Make idempotency recover the original reservation OR reject a mismatched retry. Today a
    repeated (resource, idempotency_key) is resolved to the existing reservation without
    comparing the requested amount, so a caller asking for a different amount receives a
    grant for the original amount and is told nothing. Idempotency silently becomes a source
    of inconsistent accounting instead of a protection against it.

    Also: expiry removes the key, so a retry after a lapsed lease creates a NEW reservation
    rather than recovering the original. That is the opposite failure, on the same key
    lifecycle, and it compounds the ambiguous-outcome issue.

  in_scope:
    - "Compare the request against the stored reservation on a repeated key: resource (already
       part of the key) AND amount"
    - "On a material mismatch, return a typed error naming the conflict — never a grant for a
       different amount"
    - "Define what 'materially different' means (amount; and any future request field)"
    - "Decide the retry-after-expiry behaviour: recover the original, or reject and require a
       fresh key — but never silently create a second reservation"
    - "Apply the same rule in the enterprise service (ON CONFLICT DO NOTHING currently returns
       the existing row without comparing the amount)"
    - "Cover the lost-response case: a retry after a transport failure with the same key must
       recover the original logical state, not a misleading failure"

  out_of_scope:
    - "Changing the idempotency key derivation at the seam (fresh key per dispatch, by design)"
    - "Idempotency for operations other than reserve"

  affected_layers:
    domain:
      - "Modified: error taxonomy — a typed conflict for key reused with a different request"
    application:
      - "Modified: service reserve — surface the conflict instead of a grant"
    infrastructure:
      - "Modified: InProcessLedger.reserve — compare amount on the by_key hit"
      - "Modified: InProcessLedger.expire_lapsed — do not silently drop the key"
      - "Modified: HttpLedger / enterprise repository — compare the requested amount on conflict"
    api:
      - "Modified: error response for the mismatch (distinct from Insufficient and Contended)"

  canonical_references:
    - module: ".pi/architecture/modules/capacity-ledger.md#idempotencykey"
    - adr:
        - ".pi/architecture/decisions/ADR-018-distributed-capacity-ledger.md"
    - evidence: "engine/src/capacity_ledger/infrastructure/in_process.rs — by_key hit returns grant_from(reservation) without comparing amount"
    - evidence: "engine/src/capacity_ledger/infrastructure/in_process.rs — expire_lapsed removes the key ('the claim is void, so the key may be retried afresh')"
    - evidence: "rigorix-enterprise core/src/capacity_ledger/infrastructure/reservation_repository.rs — ON CONFLICT DO NOTHING then find_by_idempotency_key, amount never compared"
    - evidence: "engine/src/capacity_ledger/domain/error.rs — no mismatch variant exists"

  acceptance_criteria:
    - "reserve(r, 4, k) then reserve(r, 999, k) returns a typed conflict, NOT a 4-unit grant"
    - "reserve(r, 4, k) twice returns the same reservation id and one reservation (existing behaviour preserved)"
    - "A retry after lease expiry does not silently create a second reservation for the same key"
    - "A lost response followed by a retry with the same key recovers the original reservation"
    - "The enterprise service enforces the same rule (an ON CONFLICT that returns an existing row must verify the payload)"
    - "The mismatch error is distinguishable from Insufficient and from Contended"

  validators:
    - ci
    - tests
    - architecture
    - canonical

  implementation_notes: |
    - Compare on the stored reservation, not on the request alone: the key's authority is the
      reservation it produced.
    - A mismatch is a caller bug, so fail loudly and type it. Returning the original grant is
      the worst option because the caller cannot tell.
    - For the enterprise, ON CONFLICT DO NOTHING must be followed by a comparison of the
      fetched row's amount with the requested amount, or the insert must be conditioned on it.
    - Retry-after-expiry is a policy decision, but 'silently new reservation' is not one of the
      acceptable answers in either direction.
    - The seam uses a fresh key per dispatch attempt by design, so it is unaffected; this is
      about callers of the port and the HTTP surface.

  file_changes:
    - "modify: engine/src/capacity_ledger/infrastructure/in_process.rs"
    - "modify: engine/src/capacity_ledger/domain/error.rs"
    - "modify: engine/src/capacity_ledger/application/service_impl.rs"
    - "create: engine/tests/capacity_ledger_idempotency_mismatch.rs"

  pipeline_steps:
    - implement
    - validate
    - create-mr
    - merge
---

# ISSUE-CAPACITY-IDEMPOTENCY-PAYLOAD-MISMATCH

## The defect

```rust
if let Some(existing_id) = state.by_key.get(&(resource.clone(), key.clone())).copied() {
    return match state.reservations.get(&existing_id) {
        Some(reservation) if reservation.state == ReservationState::Outstanding => {
            Ok(Self::grant_from(reservation))      // <-- amount never compared
        }
        ...
```

`reserve(r, 4, k)` followed by `reserve(r, 999, k)` returns a **4-unit grant** and no error.
The caller believes 999 units are reserved.

## Two failures on one key lifecycle

| case | today | required |
|---|---|---|
| same key, **different amount** | silently returns the original grant | typed conflict |
| same key, retried after expiry | key was deleted → a **new** reservation is created | recover original, or reject |

The second is the mirror image of the first: one silently under-reports the request, the other
silently over-reserves. Both make idempotency an accounting hazard.

## Evidence

| claim | evidence |
|---|---|
| no amount comparison | `in_process.rs` — `by_key` hit → `grant_from(reservation)` |
| expiry deletes the key | `in_process.rs` — `expire_lapsed` → `by_key.remove(...)`, comment: *"the claim is void, so the key may be retried afresh"* |
| no mismatch error exists | `domain/error.rs` — `Insufficient / Contended / Unavailable / LeaseExpired` |
| enterprise has the same hole | `reservation_repository.rs` — `ON CONFLICT DO NOTHING` then fetch-by-key; amount bound but never compared |

## Why it matters

The port's own contract says a repeated key "yields the same reservation and never
over-reserves" (I2). Returning a grant for a *different* amount satisfies the letter and
violates the intent: the caller's accounting and the ledger's disagree, silently.

## Acceptance Criteria

See front matter. Load-bearing: **`reserve(r, 4, k)` then `reserve(r, 999, k)` must not
return a 4-unit grant**, and a retry after expiry must not silently create a second
reservation.
