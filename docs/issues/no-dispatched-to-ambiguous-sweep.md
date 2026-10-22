---
guardian_issue:
  id: "ISSUE-NO-DISPATCHED-TO-AMBIGUOUS-SWEEP"
  title: "nothing reconciles Dispatched records after a crash: may_retry only blocks Ambiguous, so a resumed run re-dispatches an effect that may already have happened"
  epic: "effect commitment"
  epic_id: "EPIC-EFFECT-COMMITMENT"
  tracking_issue: 1164
  component: "EffectCommitment"
  module: "effect-commitment"
  status: implemented
  priority: high
  created_at: "2026-10-23T08:00:00Z"
  updated_at: "2026-10-23T14:00:00Z"

  intent: |
    Add a startup reconciliation that moves `Dispatched` records past their deadline to `Ambiguous`,
    so the fence covers the one window it currently does not.

    A process that dies between the write-ahead `begin_effect` and the settle leaves the record
    `Dispatched` forever. `DefaultRetryFence::may_retry` blocks `Ambiguous` and settled records — it
    does **not** block `Dispatched`, correctly, because a live dispatch must not be retried
    concurrently. But nothing distinguishes "in flight right now" from "in flight when the process
    died".

    Consequence: a resumed run re-dispatches an effect that may already have reached the provider —
    the exact double-spend path ADR-019 exists to close, reopened by the crash case.

    The DR plan already assumes the fix exists. `engine/docs/dr-plan-effect-commitment.md:41` says
    "Re-query the ambiguous queue. Every unsettled effect remains fenced" — but a `Dispatched` record
    is not in the ambiguous queue, so the documented recovery step cannot see it.

  dependencies:
    - name: "EffectState / EffectRecord"
      type: internal
      note: "Dispatched is non-terminal; the sweep is the only legal way out of it besides settle."
    - name: "DefaultRetryFence::may_retry"
      type: internal
      note: "Blocks Ambiguous and terminal; Dispatched is allowed — by design, and that is what the sweep must bound."
    - name: "ISSUE-CAPACITY-KEY-LIFECYCLE-UNALIGNED (#1163)"
      type: internal
      note: "The same crash window from the capacity side; both must close for the window to be shut."

  in_scope:
    - "A durable dispatch deadline (set at begin_effect, or derived from a configured lease)"
    - "A startup sweep: Dispatched older than the deadline ⇒ Ambiguous, with a reason recorded"
    - "Idempotent under concurrent roots (N replicas must not double-sweep into conflicting states)"
    - "The sweep's own observability: count swept, count skipped"
    - "DR plan and runbook updated if the sweep changes the recovery steps"
    - "Test: a Dispatched record past its deadline becomes Ambiguous; a fresh one is untouched"

  out_of_scope:
    - "Resolving Ambiguous records (still requires provider evidence — F3)"
    - "Cross-process durability of the ledger itself (enterprise L2)"
    - "Changing may_retry's contract"

  affected_layers:
    domain:
      - "New: a deadline concept on the record, or a documented derivation"
      - "Existing: EffectState::Ambiguous transition (already legal from Dispatched)"
    application:
      - "New: a reconciliation use case invoked at composition-root startup"
    infrastructure:
      - "Modified: ledger adapter — a query for expired Dispatched records"
    api:
      - "None (in_memory); the http ledger may need a list endpoint if the sweep runs service-side"

  canonical_references:
    - module: ".pi/architecture/modules/effect-commitment.md"
    - adr:
        - ".pi/architecture/decisions/ADR-019-effect-commitment.md"
    - last_sync: "2026-10-23"
    - evidence: "effect_commitment/domain/state.rs — Dispatched non-terminal; Ambiguous reachable from it"
    - evidence: "effect_commitment/application/ — no sweep/reconcile path exists (grep)"
    - evidence: "engine/docs/dr-plan-effect-commitment.md:41 — 'Re-query the ambiguous queue'"
    - evidence: "execution_engine/application/service_impl/dispatch.rs — the only begin_effect call sites"

  acceptance_criteria:
    - "A Dispatched record older than the deadline becomes Ambiguous at startup"
    - "A Dispatched record inside the deadline is untouched"
    - "Two roots sweeping concurrently do not produce conflicting transitions"
    - "The sweep records why it moved each record (deadline exceeded, not provider evidence)"
    - "The DR plan's recovery step can see the swept records"
    - "validate-ci.sh, validate-tests.sh, validate-integration.sh pass"

  validators:
    - architecture-validator
    - ci
    - tests
    - integration

  implementation_notes: |
    A sweep that marks records `Ambiguous` is asserting uncertainty, not failure — say so in the
    reason field. Do not mark them `NotCommitted`: that would claim the effect did not happen, which
    is exactly what nobody knows.

    Deadline source matters. Deriving it from a config lease (`lease_seconds`, as the capacity ledger
    does) is better than a timestamp fixed at begin: a run paused beyond the lease is already
    ambiguous by definition, and the two contexts then agree on what "too long" means.

    The sweep must be safe under N replicas. The capacity ledger settled this shape already: a
    conditional transition (`WHERE state = 'dispatched' AND started_at < deadline`) with
    `FOR UPDATE SKIP LOCKED`, not a read-then-write.

    Do not run the sweep on a timer *and* at startup in the first version. Startup is what the crash
    case needs; a periodic sweep is a second mechanism with its own timing questions. Add it later
    with evidence that startup-only is insufficient.

  file_changes:
    - "modify: engine/src/effect_commitment/domain/ (deadline or derivation)"
    - "create: engine/src/effect_commitment/application/reconcile.rs"
    - "modify: engine/src/effect_commitment/application/setup.rs"
    - "modify: cli/src/cli_boundary/orchestrator.rs, mcp/src/host/mod.rs, actions/src/main.rs (invoke at startup)"
    - "create: engine/tests/effect_commitment_reconcile.rs"
    - "modify: engine/docs/dr-plan-effect-commitment.md, engine/docs/runbook-effect-commitment.md"

  pipeline_steps:
    - implement
    - validate
    - create-mr
    - merge
---

# ISSUE-1162: `Dispatched` is a state with no exit on crash

## What you are looking at

`begin_effect` writes the record before the effect, then the process may die. The record stays
`Dispatched`. `may_retry` blocks `Ambiguous` and terminal states but not `Dispatched` — correctly, so
two live workers do not both dispatch. Nothing, however, separates "in flight now" from "in flight
when the process died", so a resumed run retries an effect that may already have happened.

The DR plan assumes the opposite: `dr-plan-effect-commitment.md:41` instructs the operator to
"re-query the ambiguous queue", which cannot contain a `Dispatched` record.

## The fix

```
begin_effect(deadline = now + lease)
   → effect happens (or the process dies)
   → startup: UPDATE ... SET state='ambiguous', reason='deadline exceeded'
              WHERE state='dispatched' AND started_at + lease < now
```

## In Scope

- Deadline, sweep at startup, idempotent under N roots, observability, DR/runbook updated, test

## Out of Scope

- Resolving ambiguous records (needs provider evidence); the enterprise store; changing `may_retry`

## Affected Layers

### Domain
- Deadline on the record, or a documented derivation from the lease

### Application
- A reconcile use case at startup

### Infrastructure
- A query for expired `Dispatched` records

### API
- None for `in_memory`

## Canonical References

- **Module:** `.pi/architecture/modules/effect-commitment.md`
- **ADR-019**
- **Evidence:** `domain/state.rs`; absence of any sweep; `dr-plan-effect-commitment.md:41`

## Acceptance Criteria

| # | Criterion | Validator |
|---|-----------|-----------|
| 1 | Expired `Dispatched` ⇒ `Ambiguous` at startup | `validate-tests.sh` |
| 2 | Fresh `Dispatched` untouched | `validate-tests.sh` |
| 3 | Concurrent roots do not conflict | `validate-integration.sh` |
| 4 | Reason records "deadline exceeded" | `validate-canonical.sh` |
| 5 | DR plan can see swept records | review |

## Implementation

1. Read `domain/state.rs`, `application/setup.rs`, `dispatch.rs` begin/settle sites, and
   `docs/dr-plan-effect-commitment.md`.
2. Add the deadline (derive from the lease if one exists).
3. Add the reconcile use case; call it from all three composition roots.
4. Conditional transition, not read-then-write.
5. Test, then update the DR plan and runbook.

## Pitfalls

- **Mark `Ambiguous`, never `NotCommitted`.** The sweep asserts uncertainty; claiming the effect did
  not happen is a claim nobody can support.
- **Derive the deadline from a lease**, so effect-commitment and capacity agree on "too long".
- **Startup only in v1.** A periodic sweep is a second mechanism with its own timing questions.
- **Do not let the sweep become a resolver.** It may only move to Ambiguous; resolution still
  requires provider evidence (F3).

## Why this is high

It is the last open window in the ADR-019 fence, and the DR plan already tells operators to look
somewhere the record cannot be found — so the documented recovery procedure silently does not apply
to the case it was written for.
