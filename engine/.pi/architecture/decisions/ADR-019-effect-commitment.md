# ADR-019: Effect commitment — owning the unsettled interval between dispatch and observed outcome

**Status:** Accepted (designed), implementation tracked by the `effect-commitment` epic
**Date:** 2026-10-20
**Supersedes:** nothing. **Superseded by:** nothing.
**Related:** ADR-017 (consequence gating), ADR-018 (capacity ledger), ADR-016 (audit ledger),
ADR-014 (effect identity), ADR-013 (write-deny boundary), ADR-011 (approval binding)

## Context

ADR-017 closes the *stale authorization* half of TOCTOU: the precondition is re-evaluated
immediately before dispatch, so an approval cannot outlive the authority it was granted under.
That is check-before-dispatch. It does not cover what happens **after** dispatch.

The uncovered window:

> A permitted action is dispatched. The response is lost. The process ends.
> Nothing records that the action was attempted.

Afterwards the system cannot distinguish two very different worlds:

| world | correct behaviour |
|---|---|
| the effect never happened | retry; the work is still owed |
| the effect happened and committed | never retry; resolve and close |

Both worlds present identically — as no record. Every downstream decision inherits that
ambiguity: retrying can double-pay, and refusing to retry can silently drop work.

### What the code does today

| fact | consequence |
|---|---|
| `NodeStatus` is `Pending, InProgress, Completed, Failed, Skipped` (`state_persistence/domain/state.rs:91`) | an unknown outcome must be recorded as **Completed or Failed** — a lie with a timestamp |
| the parallel dispatch loop holds `sessions: Arc<Mutex<HashMap>>` (`service_impl.rs:86`) and persists at start, pause and final only | the dispatch window has no durable record |
| an approval pause **is** durable — the orchestrator saves graph + node states (`orchestrator_impl.rs:1045`) and `approval_flow.rs:37` hydrates from `load_state` | the gap is specifically *after dispatch*, not *while paused*; this distinction must be stated or the two are conflated |
| `ReservationState::Ambiguous` exists and is deliberately non-terminal (`capacity_ledger/domain/reservation.rs:104,116`) | the right *shape* already exists — but it proves **custody of capacity**, not **commitment of an effect** |
| the capacity idempotency key is attempt-derived, `{dag_id}:{node_id}:{uuid}` (`dispatch.rs:795`) | a retry is a **new intent**, so a same-key read is impossible and no fence can be built |

The last row is the one that makes this architectural rather than a patch. The custody property
everyone reaches for — *a retry reads the existing intent instead of dispatching again* — requires
the key to identify the **intent**, not the **attempt**. The engine currently does the opposite,
deliberately, to avoid the silent-collision bug fixed in #1052.

## Decision

Introduce a bounded context, **`effect-commitment`**, that owns the unsettled interval.

It is a context, not an edit, because it owns a decision no existing context owns, has its own
language, and takes its authority from **outside the system**:

| criterion | `effect-commitment` | nearest existing owner | why it cannot own it |
|---|---|---|---|
| decision | "was this effect committed?" | execution-engine | knows *dispatch*, not commitment |
| | | audit | records *claims*; has nothing to record in this window |
| | | capacity-ledger | knows *reservation*; a hold is not a payment |
| language | intent, dispatched, ambiguous, observed outcome, resolver, receipt, fence | — | none of these belong to `NodeStatus` or `ReservationState` |
| authority | **provider receipt / read-back** | — | every existing context's authority is internal |
| invariant | an unsettled effect cannot be retried, closed, or settled by the party that lost the response | — | stated nowhere today |
| port | `EffectCommitmentLedger` + `ProviderReceiptReader` (anti-corruption over a foreign system) | — | new dependency shape |

### Normative state machine

```
Intent --dispatch-----------------------> Dispatched
Dispatched --observed success-----------> Committed      (terminal)
Dispatched --observed definite failure--> NotCommitted   (terminal)
Dispatched --lost response / deadline / crash --> Ambiguous   (NOT terminal)
Ambiguous --resolver + provider evidence--> Committed | NotCommitted
```

Illegal, and rejected in the type or the policy layer:

- `Ambiguous -> Dispatched` — a retry while unsettled
- leaving `Ambiguous` without a recorded resolver identity and evidence digest
- closing the effect because a lease lapsed (lapse is absence of evidence, not evidence of absence)

### Fencing rules (normative)

| id | rule |
|---|---|
| F1 | dependents of an `Ambiguous` effect are not released |
| F2 | a retry of an `Ambiguous` effect is refused; the run stays open |
| F3 | only a resolver presenting provider evidence may leave `Ambiguous` |
| F4 | the dispatching process and the agent are never the resolver; the deciding artifact must resolve outside the agent-writable workspace (ADR-013) |

### Intent-derived key

`IdempotencyKey = H(dag_id, node_id, intent_digest)` where `intent_digest` covers the tool, its
arguments, and the precondition/authority digests already produced by the gate.

Stable across retries, changes only when the intent changes. This restores the same-key read
without reintroducing #1052: same key + same payload returns the existing record; same key +
different payload is refused.

### The port (normative)

```
begin(intent) -> Dispatched
settle_definite(reservation_id, outcome) -> Committed | NotCommitted
mark_ambiguous(reservation_id, reason) -> Ambiguous
resolve(reservation_id, evidence, resolver) -> Committed | NotCommitted
query(reservation_id | dag_id) -> EffectRecord(s)
```

`begin` MUST be durable before the effect is sent. If it is not, the context has not been entered
and the guarantee does not exist — this is the write-ahead requirement and the one that cannot be
relaxed.

## Consequences

**Positive**

- The system can answer "did it happen?" with `never attempted`, `attempted and unresolved`, or
  `observed` — instead of collapsing the first two into silence.
- Retry becomes safe: same intent ⇒ same key ⇒ read, not re-send.
- The audit chain gains a record for the interval it currently cannot see.

**Negative, and accepted**

- A run can block indefinitely on `Ambiguous`. That is correct: an unsettled payment must block
  closure. Operability therefore requires an ambiguous-effects query and an operator resolve path,
  not a timeout that guesses.
- Deployments with no receipt reader can only ever produce `Ambiguous`. They can prove
  *not-settled*; they can never prove *settled*. This must be stated in the runbook, not discovered.
- One new port and one new adapter per provider.

## Rejected alternatives

| alternative | why rejected |
|---|---|
| **A. Add `Ambiguous` to `NodeStatus` and stop** | durability without reconciliation is a lie with a timestamp. The state exists to be *left*, by an authority — if nothing can leave it, it is a tombstone |
| **B. Reuse `ReservationState::Ambiguous`** | it proves capacity is held, not that an effect committed. Conflating them makes a capacity record read as proof of payment — precisely the class of defect this series keeps finding |
| **C. Let the process that lost the response settle it** | it is the one party that provably cannot know. Violates F4 and the ADR-013 trust boundary |
| **D. Keep the attempt-derived key** | a retry becomes a new intent, so `F2` is unenforceable and the custody invariant cannot hold |

## Naming

`effect-commitment` rather than `outcome-ledger` or `settlement`: the context owns one question —
*was this effect committed?* — and "commitment" is the word the capacity ledger already uses for
the terminal, proven-success state. Sharing the vocabulary is intended; sharing the record is not.

## Claim ceiling

- **L1 (in-process):** the record is written before dispatch, the fence holds, and an `Ambiguous`
  effect blocks retry and dependents. Testable without any provider.
- **L2 (cross-process):** the record and its transitions survive process death and are settled
  through a contested store (HTTP adapter), with the same fencing. Requires the adapter.
- **L3 (commitment):** "the payment committed" requires provider receipt / read-back. Without that
  integration the engine can prove **not-settled**, never **settled**. No deployment may claim L3
  while its resolver has no provider evidence.
