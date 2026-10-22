---
guardian_issue:
  id: "ISSUE-RECONCILE-SWEEP-BELONGS-TO-THE-SERVICE"
  title: "the Dispatched sweep runs in every client root against the local clock; at L2 the service must own it, or roots fence each other's live work"
  epic: "effect commitment"
  epic_id: "EPIC-EFFECT-COMMITMENT"
  tracking_issue: 1173
  component: "EffectCommitment"
  module: "effect-commitment"
  status: planned
  priority: medium
  created_at: "2026-10-23T13:00:00Z"
  updated_at: "2026-10-23T13:00:00Z"

  intent: |
    Keep the client-side sweep for the in-process backend only, and require the ledger service to
    own reconciliation once `backend.kind = "http"`.

    Today `EffectReconciler::reconcile` runs at startup in all three composition roots
    (`cli`, `mcp`, `actions`). That is correct at L1: there is no other actor, and the sweep is the
    only thing that closes the crash window.

    At L2 it becomes wrong in two ways, both silent:

    1. **It duplicates the service.** ADR-011 decides the sweep belongs to the ledger service, because
       a fleet of roots would each sweep a state they cannot see. Two implementations of one
       invariant is how they diverge.
    2. **It compares clocks across processes.** The predicate is
       `record.started_at + lease < Utc::now()`, where `started_at` was written by whichever process
       dispatched and `Utc::now()` is the sweeping process's clock. A root whose clock runs fast marks
       *live* dispatches `Ambiguous` — falsely fencing work that is still running, which the operator
       reads as a double-spend risk. A slow clock leaves orphans unswept. Both are silent, and a
       15-minute lease only hides the skew below fifteen minutes.

    There is a third, smaller cost: `query(EffectQuery::dispatched(None))` fetches every dispatched
    record and filters client-side, so each root pulls the whole in-flight set at startup and issues
    one `mark_ambiguous` round-trip per expired record.

  dependencies:
    - name: "EffectReconciler"
      type: internal
      note: "application/reconcile.rs — reconcile() and DEFAULT_DISPATCH_LEASE (15 min)."
    - name: "three composition roots"
      type: internal
      note: "cli/src/cli_boundary/orchestrator.rs:380, mcp/src/host/mod.rs:1616, actions/src/main.rs:433 — all call reconcile() and propagate the error."
    - name: "ADR-011 (enterprise)"
      type: external
      note: "Decides DispatchedSweeper lives in the service. This issue records the OSS-side obligation that follows from it."

  in_scope:
    - "Run the client sweep only when the configured backend is in-process"
    - "When the backend is http, either skip the sweep or call a service endpoint that owns it"
    - "Make the expiry predicate evaluate the deadline on the store's clock, not the caller's (for the in-process backend: one clock, documented; for http: the service decides)"
    - "Document the clock assumption where an operator will read it"
    - "Test: an http-configured root does not sweep locally; an in-process root still does"

  out_of_scope:
    - "Implementing the service-side sweeper (ADR-011 epic)"
    - "Changing the 15-minute default in this issue"
    - "Periodic sweeping; startup remains the trigger"

  affected_layers:
    domain:
      - "None — `is_dispatch_expired` stays the predicate, but its inputs must have one clock"
    application:
      - "Modified: EffectReconciler — backend-aware, and explicit about whose clock decides"
    infrastructure:
      - "Modified: a capability flag on the ledger (can it sweep?) rather than a branch on a config string"
    api:
      - "None in OSS; the service endpoint is ADR-011's"

  canonical_references:
    - module: ".pi/architecture/modules/effect-commitment.md"
    - adr:
        - ".pi/architecture/decisions/ADR-019-effect-commitment.md"
    - last_sync: "2026-10-23"
    - evidence: "effect_commitment/application/reconcile.rs:38 — DEFAULT_DISPATCH_LEASE = 15 min"
    - evidence: "effect_commitment/application/reconcile.rs:73-80 — Utc::now() compared with record.started_at"
    - evidence: "effect_commitment/application/reconcile.rs:74 — query(dispatched) then filter client-side"
    - evidence: "cli/orchestrator.rs:380, mcp/host/mod.rs:1616, actions/main.rs:433 — three call sites"
    - evidence: "enterprise ADR-011 — DispatchedSweeper belongs to the service"

  acceptance_criteria:
    - "An http-configured root performs no local sweep"
    - "An in-process root still sweeps expired Dispatched records"
    - "The predicate's clock source is documented, and for http the decision is the service's"
    - "The behaviour is expressed as a ledger capability, not a string comparison on config"
    - "The DR plan states which component reconciles at each ceiling"
    - "validate-ci.sh, validate-tests.sh, validate-canonical.sh pass"

  validators:
    - architecture-validator
    - ci
    - tests
    - canonical

  implementation_notes: |
    A capability on the ledger (`fn can_sweep(&self) -> bool`, or `reconcile` living behind a trait
    method the in-process adapter implements and the http adapter does not) is better than branching
    on `backend.kind == "http"` in three roots. A string comparison in the composition root is the
    kind of thing that is correct until someone adds a third backend kind.

    Do **not** delete the client sweep. Until the service exists, it is the only thing that closes
    the window at L1, and a client that stops sweeping because a config key says http — while the
    service does not sweep either — is worse than the current state.

    The clock point is the one to write down. "Whose clock decides whether a dispatch is expired?" has
    a different answer per ceiling, and an operator debugging a falsely-ambiguous record needs it in
    the runbook rather than in a comment.

  file_changes:
    - "modify: engine/src/effect_commitment/application/reconcile.rs"
    - "modify: engine/src/effect_commitment/application/ledger.rs (capability)"
    - "modify: engine/src/effect_commitment/infrastructure/http.rs, in_memory.rs"
    - "modify: cli/src/cli_boundary/orchestrator.rs, mcp/src/host/mod.rs, actions/src/main.rs"
    - "modify: engine/docs/runbook-effect-commitment.md, dr-plan-effect-commitment.md"
    - "create: engine/tests/effect_commitment_reconcile_backend.rs"

  pipeline_steps:
    - implement
    - validate
    - create-mr
    - merge
---

# ISSUE-1173: whoever sweeps must own the clock

## What you are looking at

`reconcile()` runs at startup in all three roots. At L1 that is the fix for #1164. At L2 — once
`backend.kind = "http"` points at the ledger service — the same call becomes both redundant
(ADR-011 puts `DispatchedSweeper` in the service) and unsound, because the predicate is:

```
record.started_at + 15min  <  Utc::now()
      ↑ written by the dispatching process      ↑ the sweeping process's clock
```

Two processes, two clocks. Fast clock ⇒ live dispatches marked `Ambiguous` (false double-spend
signal). Slow clock ⇒ orphans stay `Dispatched` forever. Both silent; the 15-minute lease only hides
skew below fifteen minutes.

## Decision (already made)

- In-process backend: the root sweeps, one clock, documented.
- Http backend: the service sweeps; the client must not.
- The predicate's clock source is stated in the runbook, because it is what an operator needs when
  asking "why is this record Ambiguous?".

## In Scope

- Backend-aware sweep (capability, not a config string)
- Clock assumption documented
- DR plan says which component reconciles per ceiling

## Out of Scope

- Implementing the service sweeper (ADR-011)
- Changing the default lease; periodic sweeping

## Affected Layers

### Domain
- `is_dispatch_expired` stays; its inputs must come from one clock

### Application
- `EffectReconciler` backend-aware

### Infrastructure
- a `can_sweep` capability on the ledger adapters

### API
- None here

## Canonical References

- **Module:** `.pi/architecture/modules/effect-commitment.md`
- **ADR-019**; enterprise **ADR-011**
- **Evidence:** `reconcile.rs:38,73-80`; the three call sites; ADR-011's `DispatchedSweeper`

## Acceptance Criteria

| # | Criterion | Validator |
|---|-----------|-----------|
| 1 | Http-configured root does not sweep locally | `validate-tests.sh` |
| 2 | In-process root still sweeps | `validate-tests.sh` |
| 3 | Clock source documented per ceiling | `validate-canonical.sh` |
| 4 | Expressed as a capability, not a string check | `validate-architecture.sh` |
| 5 | DR plan names the reconciler per ceiling | review |

## Implementation

1. Read `application/reconcile.rs`, the three call sites, and ADR-011's sweeper section.
2. Add the capability to the ledger port; implement it in both adapters.
3. Gate the sweep on it; document the clock source.
4. Test both configurations; update DR plan and runbook.

## Pitfalls

- **Do not delete the client sweep.** At L1 it is the only thing closing the window.
- **Do not branch on `backend.kind == "http"` in three roots** — a capability is the same fix with
  one place to get wrong.
- **Do not shrink the lease to "make skew impossible"** — it would sweep live long dispatches.
- **State the clock answer for each ceiling**, or the next operator debugs a phantom.

## Why this is medium

Nothing is wrong at L1, which is what ships. But the moment the L2 backend is configured, three
roots and a service race to reconcile the same records with different clocks — and the symptom is a
record marked `Ambiguous` while its effect is still running.
