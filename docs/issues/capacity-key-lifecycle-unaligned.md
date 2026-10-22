---
guardian_issue:
  id: "ISSUE-CAPACITY-KEY-LIFECYCLE-UNALIGNED"
  title: "capacity holds use a per-attempt UUID while the effect record uses an intent key: a crash after reserve leaves an unrecoverable hold, and a resumed dispatch holds twice"
  epic: "effect commitment"
  epic_id: "EPIC-EFFECT-COMMITMENT"
  tracking_issue: 1163
  component: "ExecutionEngine"
  module: "capacity-ledger"
  status: planned
  priority: high
  created_at: "2026-10-23T08:00:00Z"
  updated_at: "2026-10-23T08:00:00Z"

  intent: |
    Derive the capacity reservation key from the same intent identity the effect record uses, so a
    retry reads the hold instead of creating a second one.

    Today `dispatch.rs:1035` mints a fresh UUID per dispatch attempt. Its comment justifies that:
    "the ledger answers `Contended` once a key is resolved, which would turn a legitimate retry into
    a refusal". **That justification is stale.** Since #1052, an idempotent retry with the same key
    and the same amount returns the *same* reservation (`capacity service_impl.rs:98`), and the guard
    registry already handles the vacant-id case (`Entry::Vacant`).

    So the two lifecycles are unaligned for no reason: the effect record is keyed by
    `H(dag_id, node_id, intent_digest)` (ADR-019) and can answer "what happened to this intent?",
    while the hold is keyed by a value nobody can recompute after a crash.

    The failure this enables, which is why it is high:
    1. Dispatch reserves capacity (hold H1, UUID key), then dies before settling.
    2. The effect record is left `Dispatched`, which `may_retry` does **not** block (see #1164).
    3. The resumed run re-dispatches, mints a new UUID, and reserves a *second* hold H2.
    4. H1 is only recovered by lease expiry, so the resource is double-held in the meantime — and the
       resumed run may be refused for capacity it already owns.

  dependencies:
    - name: "IntentKeyDerivation (ADR-019)"
      type: internal
      note: "effect_commitment/application/key.rs — the key the record already uses."
    - name: "capacity idempotent retry (#1052)"
      type: internal
      note: "capacity_ledger/application/service_impl.rs:98 — same key + same amount ⇒ same reservation id."
    - name: "ISSUE-NO-DISPATCHED-TO-AMBIGUOUS-SWEEP (#1164)"
      type: internal
      note: "The other half of the same crash window; fix both or the window stays open from one side."

  in_scope:
    - "Derive the reservation key from (dag_id, node_id, intent_digest) using the existing derivation"
    - "Keep the amount part of the identity: same key with a different amount must be refused, not silently reused"
    - "Delete or correct the stale comment at dispatch.rs:1035"
    - "Test: a re-dispatch of the same intent reuses the existing hold; a changed amount is refused"

  out_of_scope:
    - "Changing the capacity ledger's error vocabulary"
    - "The Dispatched→Ambiguous sweep (#1164)"
    - "Cross-process durability of the hold (enterprise L2)"

  affected_layers:
    domain:
      - "None — the intent derivation exists"
    application:
      - "Modified: dispatch.rs capacity reserve call"
    infrastructure:
      - "None"
    api:
      - "None"

  canonical_references:
    - module: ".pi/architecture/modules/capacity-ledger.md"
    - adr:
        - ".pi/architecture/decisions/ADR-018-distributed-capacity-ledger.md"
        - ".pi/architecture/decisions/ADR-019-effect-commitment.md"
    - last_sync: "2026-10-23"
    - evidence: "execution_engine/application/service_impl/dispatch.rs:1028-1035 — UUID key and its justification"
    - evidence: "capacity_ledger/application/service_impl.rs:98-112 — idempotent retry returns the same reservation; vacant-id guard only"
    - evidence: "effect_commitment/application/key.rs:6 — IdempotencyKey = H(dag_id, node_id, intent_digest)"

  acceptance_criteria:
    - "A second dispatch of the same intent returns the existing reservation, not a new one"
    - "A second dispatch with a different amount is refused, and the refusal is distinguishable"
    - "No code comment claims the ledger refuses a repeated key"
    - "Test covers reuse and amount-change"
    - "validate-ci.sh, validate-tests.sh, validate-architecture.sh pass"

  validators:
    - architecture-validator
    - ci
    - tests
    - architecture

  implementation_notes: |
    The amount must stay in the identity. A same-key/different-amount request is a caller bug, not a
    retry, and quietly returning the old hold would under-reserve. Decide and document which error
    that maps to; do not fold it into `Insufficient`.

    Do not reuse the *effect* key storage for the hold. Same derivation, independent rows — the two
    contexts stay separate even when they share an identity.

  file_changes:
    - "modify: engine/src/execution_engine/application/service_impl/dispatch.rs"
    - "create: engine/tests/capacity_intent_key_reuse.rs"

  pipeline_steps:
    - implement
    - validate
    - create-mr
    - merge
---

# ISSUE-1161: capacity and effect keys describe different things

## What you are looking at

`dispatch.rs:1035` mints `IdempotencyKey::new(format!("{dag_id}:{node_id}:{Uuid::new_v4()}"))` per
attempt, because the comment says a deterministic key would collide with the previous attempt's
reservation and be answered `Contended`. That was true before #1052; it is not true now — a same-key
same-amount retry returns the same reservation (`service_impl.rs:98`), and the guard registry only
inserts on a vacant id.

So the hold cannot be found after a crash, while the effect record can. The two lifecycles should
share one identity.

## The crash chain

```
dispatch → reserve (hold H1, UUID key) → process dies
effect record: Dispatched        (may_retry does not block it — #1164)
resumed run → re-dispatch → new UUID → reserve (hold H2)
                                    → resource double-held until H1's lease expires
                                    → and the resumed run may be refused for capacity it owns
```

## In Scope

- Key from `(dag_id, node_id, intent_digest)`; amount stays part of the identity
- Remove the stale justification
- Test reuse and amount-change

## Out of Scope

- Ledger error vocabulary; the sweep (#1164); cross-process durability

## Affected Layers

### Domain
- None; the derivation exists

### Application
- `dispatch.rs` reserve call

### Infrastructure / API
- None

## Canonical References

- **Module:** `.pi/architecture/modules/capacity-ledger.md`
- **ADR-018 / ADR-019**
- **Evidence:** `dispatch.rs:1028-1035`, `capacity service_impl.rs:98-112`, `effect key.rs:6`

## Acceptance Criteria

| # | Criterion | Validator |
|---|-----------|-----------|
| 1 | Same intent ⇒ same hold | `validate-tests.sh` |
| 2 | Different amount ⇒ refused, distinguishably | `validate-tests.sh` |
| 3 | No comment claims a repeated key is refused | `validate-canonical.sh` |
| 4 | Layer boundaries | `validate-architecture.sh` |

## Implementation

1. Read `dispatch.rs:1015-1060`, `capacity service_impl.rs:85-115`, `effect key.rs:1-40`.
2. Derive the key with the existing derivation; keep amount in the identity.
3. Fix the comment.
4. Add the two tests.

## Pitfalls

- **Keep the amount in the identity.** Same key + different amount is a caller bug; returning the
  old hold would under-reserve.
- **Same derivation, independent rows.** Sharing an identity is not sharing storage.
- **Do not widen the capacity error taxonomy** to encode this; pick an existing variant and document.

## Why this is high

It is the difference between "the system can find the capacity it already reserved" and "it cannot",
and it only shows up after a crash — when the operator has least ability to work it out.
