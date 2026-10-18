# ADR-018: Distributed capacity ledger — cross-session aggregate enforcement

**Status:** Accepted (designed), implementation tracked by the `capacity-ledger` epic
**Date:** 2026-10-14
**Supersedes:** nothing. **Superseded by:** nothing.
**Related:** ADR-008 (RAII reservation), ADR-014 (effect identity), ADR-016 (audit ledger),
ADR-013 (write-deny boundary), ADR-017 (consequence gating)

## Context

A single execution's spend can be capped today. **Aggregate exposure across independent
executions cannot.** Five sessions that each stay under a per-session cap can jointly exceed
the cap that matters. A pre-check followed by an increment ("naive counter") does not fix this:
under concurrency, two executions both read the same "room available" and both proceed.

The engine today has **two parallel budget mechanisms**, and neither answers the aggregate
question:

| component | shape | limitation |
|---|---|---|
| `budget_tracking` — `LlmBudget` + RAII `LlmBudgetReservation` | real reserve → commit/rollback, `Arc<BudgetState>` | **LLM-scoped**; state is **in-process** and **ephemeral** — the `LlmBudgetRepository` persistence port had no caller and was deleted (#1016) |
| `enforcement` — `ResourceBudget { resource, soft_warning_threshold, hard_limit, current_usage }` | generic **counter** + threshold evaluation | a **check, not a reservation** — exactly the insufficient "naive counter" |

Read against the published requirement (a shared cross-session risk ledger with
**reserve-then-authorize** semantics), the gap is not the *reservation lifecycle* — that is
proven and shipped — it is **atomicity across independent executions**: a durable, contested,
shared store, plus the lease/idempotency semantics that make it safe.

## Decision

Introduce **one new bounded context in the engine: `capacity_ledger`**, owning the *model*
and the *port* — **not the storage**.

1. **The engine owns the contract and the decision; the storage owns the atomicity.**
   The engine has no database dependency and will not acquire one. A port
   (`CapacityLedger`) expresses the atomic operations; adapters implement it.

2. **OSS stays standalone with no DB.** The default adapter is `InProcessLedger`
   (single-execution semantics). The ledger is **opt-in per resource**: absent configuration,
   the engine behaves exactly as today — no database, no network, no shared state.

3. **Distributed mode is an enterprise service, reached over HTTP.** Because the enterprise
   does not depend on `rigorix-engine` (verified: no such dependency), the coupling is the
   **wire protocol**, matching the existing execution-plane / control-plane split and the
   existing external-HTTP-call discipline (`precondition/infrastructure/provider_client.rs`).
   The enterprise owns a Postgres-backed `capacity_ledger` service; the engine's
   `HttpLedger` adapter calls it.

4. **The store is the source of truth in distributed mode.** No in-memory reservation state
   may be authoritative when a shared ledger is configured — otherwise N replicas diverge.

## The port (normative)

```rust
async fn reserve(&self, req: ReserveRequest) -> Result<Grant, LedgerError>;
async fn commit(&self, grant: &Grant, actual: u64) -> Result<(), LedgerError>;
async fn release(&self, grant: &Grant) -> Result<(), LedgerError>;
async fn exposure(&self, q: &ExposureQuery) -> Result<Exposure, LedgerError>;
```

Invariants — each is a real failure mode, not a formality:

| # | Invariant | Why |
|---|---|---|
| I1 | **Atomic & linearizable** per (resource, window) | the storage serialises; the engine never attempts this locally |
| I2 | **Idempotent** on `idempotency_key` | distribution means retries; without this, a retry *becomes* the over-spend |
| I3 | **Leased** — every reservation carries `expires_at` | a dead holder must not strand capacity |
| I4 | **Fail-closed** — unreachable/timeout/ambiguous ⇒ refuse | a hard cap that fails open is not a cap; matches `failure = "deny"` |
| I5 | **Store clock**, not caller clock | lease expiry on distributed clocks is a known hazard |
| I6 | **No claim inflation** — the adapter's guarantee is stated, never implied | an `InProcessLedger` gives single-execution atomicity only |

## Consequences

**Positive**
- The aggregate-exposure requirement becomes answerable with a real reservation, not a counter.
- One coherent home for contested capacity; removes the need to bend `enforcement` into a
  stateful component or to make the LLM-scoped `budget_tracking` generic in place.
- OSS standalone guarantee is preserved structurally (opt-in + `InProcessLedger` default).

**Negative / accepted**
- A new context, a new port, and a new enterprise service — real surface area.
- **Fail-closed makes the control plane a critical path for a configured cap.** If the ledger
  is unreachable, execution stops. Batching/leasing capacity to reduce round-trips would
  reintroduce local counters; that trade-off is deliberately *not* taken now.
- The hard part is the adapter's concurrency semantics (stale leases, crash recovery, retries),
  which is design work, not just implementation.

## Rejected alternatives

| Alternative | Rejected because |
|---|---|
| Generalise `budget_tracking` in place | leaves a context named for LLM cost owning distributed coordination — two reasons to change, and a name that lies |
| Extend `enforcement.ResourceBudget` into a reservation | makes a stateless evaluator stateful, and still cannot provide cross-process atomicity |
| Put `sqlx`/Redis in the engine | forces every OSS consumer to run a database; breaks the storage-free engine |
| Redis as the ledger store | the point is durability across restarts and rolling deploys; Redis stays for rate limiting |
| File-lock adapter as the "distributed" answer | single-filesystem reach only; unsafe over shared network volumes; a dev convenience, not a topology |

## Naming

**"Ledger" in the enterprise already means the hash-chained audit ledger** (ADR-016:
`producer_id`, `ledger_sequence`, `prev_hash`, `history_integrity`). The two are distinct and
must not be conflated:

- **Audit ledger** — append-only, immutable, the *proof* of what happened.
- **Capacity ledger** — mutable, leased, the *live* accounting of contested capacity.

## Claim ceiling

With `InProcessLedger`: **single-execution atomicity**. With `HttpLedger` + the enterprise
service: **cross-session/multi-replica atomicity**, and only when the configured store is a
real contested store. No deployment may claim the second while running the first. This
distinction is load-bearing for docs and for the demo.
