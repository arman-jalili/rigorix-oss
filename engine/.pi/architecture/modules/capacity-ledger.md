# Module: capacity_ledger

**Bounded context:** `capacity_ledger` (new — ADR-018)
**Layer:** engine (`engine/src/capacity_ledger/`)
**Status:** planned

## Responsibility

Own the **model and the port** for contested, limited capacity shared across independent
executions: what a resource is, what a reservation is, what a lease is, what the aggregate
exposure is, and which atomic operations the engine may perform against them.

It explicitly does **not** own storage. Storage is an adapter, because the engine has no
database dependency and must not acquire one.

## Why a separate context

| candidate host | why not |
|---|---|
| `budget_tracking` | scoped and named for LLM cost; a real reserve/commit/rollback implementation lives here, but its state is in-process and its language is tokens |
| `enforcement` | a stateless evaluator (`ResourceBudget` is a counter with a threshold); making it stateful adds a second reason to change |
| `sequence_policy` | answers *composition* ("have we done this before?", via `prior_actions` and effect keys), not *capacity* ("is there room left?") |

The new context is the only one whose reason to change is **distributed coordination**:
atomicity, idempotency, leases, reclamation.

## Layers

```
engine/src/capacity_ledger/
  domain/          Resource · Reservation · Grant · Lease · Exposure · LedgerError
  application/     reserve · commit · release · exposure   (use cases over the port)
  interfaces/      trait CapacityLedger                     (the contract)
  infrastructure/
    in_process.rs  InProcessLedger   — default; single-execution semantics
    http.rs        HttpLedger        — opt-in; calls the enterprise ledger service
```

## The contract

See ADR-018 §"The port (normative)" for the signatures and invariants I1–I6.
The invariants are the module's actual specification; the signatures are incidental.

## Consumers

- **`enforcement`** — before permitting, `reserve`; on `Insufficient` refuse; after the
  effect `commit`; on failure/drop `release` (RAII, the ADR-008 pattern already proven in
  `budget_tracking`). Wiring is tracked by its own issue and is verified by a
  **reachability test**, not by unit tests alone.
- **`budget_tracking`** — *optionally, later*: become a consumer of this port so OSS has one
  budget system instead of two. Staged deliberately; the existing code is proven.

Unchanged: `sequence_policy`. Composition and capacity remain separate mechanisms.

## Configuration (opt-in)

A resource is only ledger-governed if declared. Absent declaration ⇒ today's behaviour.
The wire/declaration surface is additive and must not change existing keys.

## Testing strategy

| level | what it proves |
|---|---|
| unit | domain invariants (lease arithmetic, exposure summation, state transitions) |
| contract | **one conformance suite run against every adapter** — `InProcessLedger` and `HttpLedger` must satisfy I1–I6 identically |
| concurrency | N concurrent reservations never exceed `hard_limit` (the naive-counter falsifier) |
| recovery | lease expiry, reclaim, commit-after-expiry, idempotent retry |
| **wiring** | the gate actually *calls* the port on the execution path — the "implemented but not wired" failure |
| e2e | multi-replica enterprise run against a real Postgres, after all parts land |

## Canonical references

- ADR-018: `.pi/architecture/decisions/ADR-018-distributed-capacity-ledger.md`
- ADR-008: RAII reservation pattern (proven in `budget_tracking`)
- ADR-016: the **audit** ledger — distinct from this one
- Enterprise counterpart: `ADR-010-capacity-ledger-service`
