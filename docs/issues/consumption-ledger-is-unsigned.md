---
guardian_issue:
  id: "ISSUE-CONSUMPTION-LEDGER-IS-UNSIGNED"
  title: "the consumption ledger that anchors approvals against rollback is an unsigned JSON map, so deleting one marker makes a spent approval look fresh"
  epic: "consequence gating"
  component: "ApprovalBinding"
  module: "approval"
  status: planned
  priority: high
  tracking_issue: 1244
  dependencies:
    - "#1241 — the anti-rollback anchor"

  intent: |
    #1241 anchors consumption outside the store: a terminal transition is written to a
    separate ledger keyed by `(node_id, nonce)`, and a pending record whose nonce is already
    terminal is refused as a rollback. That closes the single-file restore.

    The ledger itself is not authenticated:

    ```rust
    struct ConsumptionMarker { node_id: Uuid, nonce: Uuid, status: ApprovalStatus, recorded_at: DateTime<Utc> }
    ```

    The file is a JSON map of those, rewritten on every terminal transition, with no MAC, no
    chain and no head. Deleting one key is a one-line edit that leaves no trace, and the
    loader then takes the same path as a *fresh* store:

    - `reject_rolled_back` refuses only when the marker map contains the nonce;
    - with the marker gone, the record verifies by its MAC and loads;
    - `a_fresh_store_without_a_marker_still_works` is the test that proves this path accepts.

    So restoring an authentic pre-consumption copy of the store — the exact attack #1241 was
    filed about — costs one extra edit, and that edit is to an unsigned file the anchor
    depends on completely.

    After this change truncation of the ledger is detected: a marker that cannot be verified
    is a refusal, not an absence.

  in_scope:
    - "The ledger's integrity is checkable: a marker set that was truncated is refused, not accepted"
    - "The distinction between 'fresh ledger' and 'ledger shortened' is explicit and recorded"
    - "The refusal names the store and the ledger path"
    - "Seeds: delete one marker, restore a valid pending record → refused; delete the whole ledger on a first run → works and says the ledger is new"

  out_of_scope:
    - "Rolling back both the store and the ledger to an older pair — no local anchor can detect that, and it needs the cross-check against the signed trail"
    - "Where the ledger lives; doctor already classifies it"
    - "The per-record MAC, which works"

  affected_layers:
    domain:
      - "Modified: the ledger's integrity — a verifiable structure, not a bare map"
    application:
      - "Modified: verify/load — truncation is an integrity failure"
    infrastructure:
      - "Modified: ledger persistence"
    api:
      - "Modified: doctor reports whether the ledger verifies, not only where it is"

  canonical_references:
    - adr: ".pi/architecture/decisions/ADR-011-approval-binding.md"
    - last_sync: "2026-10-31"
    - evidence: "engine/src/approval/infrastructure/repository/approval_repository_impl.rs — ConsumptionMarker has no MAC; the ledger is rewritten as a JSON map"
    - evidence: "reject_rolled_back refuses only when `consumed.contains_key(marker_key(node_id, nonce))`"
    - evidence: "engine/tests/approval_store_rollback_integration.rs — a_fresh_store_without_a_marker_still_works proves the marker-absent path accepts"
    - evidence: "measured: deleting the single marker leaves doctor reporting the same verification count and no error"

  acceptance_criteria:
    - "Removing a marker from an existing ledger is refused when a record for that nonce is presented as pending"
    - "A first run with no ledger still works, and reports that the ledger has no history yet"
    - "doctor reports whether the ledger verified, alongside its path and classification"
    - "Seeds fail if the truncation check is removed"

  validators:
    - ci
    - tests
    - security

  implementation_notes: |
    - #1237 established that the store needs a MAC because it is the authority for "may this
      dispatch happen". The ledger is now that authority one step stronger, and it has the
      protection the store had before #1237.
    - A per-entry MAC does not solve this: deleting an entry removes its MAC with it. The
      structure must make truncation visible — a head, a count, or a cross-check against the
      envelope trail, which already records each approval and is signed.
    - Keep "no ledger yet" working. A refusal on first use would make the fix unusable, and
      the distinction between absent and shortened is the whole design problem.

  file_changes:
    - "modify: engine/src/approval/infrastructure/repository/approval_repository_impl.rs"
    - "modify: engine/src/approval/application/service_impl.rs"
    - "modify: cli/src/cli_boundary/doctor.rs"
    - "modify: engine/tests/approval_store_rollback_integration.rs"

  pipeline_steps:
    - implement
    - validate
    - create-mr
    - merge
---

# ISSUE-CONSUMPTION-LEDGER-IS-UNSIGNED

## The anchor and its lock

```
store (MAC'd, verifies)        ← restored to an older authentic copy: refused
ledger (plain JSON)            ← one key deleted: no trace, no refusal
```

`reject_rolled_back` refuses a pending record only when the marker is present:

```rust
if consumed.contains_key(&marker_key(record.node_id, record.nonce)) {
    return Err(ApprovalError::RecordRollback { node_id, step_name });
}
Ok(())
```

With the marker gone, the record is a MAC-valid pending record in a store with no marker for
it — which is byte-for-byte the state `a_fresh_store_without_a_marker_still_works` accepts.

Measured: deleting the single marker from a live ledger left `doctor` reporting the same
record count and no error, because nothing in the design can tell "no consumption yet" from
"the record of it was removed".

## Why it matters more than the store did

The store needed a MAC because it answers "may this dispatch happen". The ledger answers
"has this approval already been spent" — the single-use property itself. An anchor that can
be emptied one key at a time restores the pre-#1241 behaviour at the cost of one more edit,
and the edit is invisible.

## The required end state

The ledger is verifiable as a whole, so a shortened ledger is an integrity failure rather
than an absence, a first run with no ledger still works and says so, and the refusal names
both paths.

## How it is proven

| seed | expected |
|---|---|
| delete one marker, present a valid pending record for that nonce | refused: truncation detected |
| delete the entire ledger on a first run | works; reports the ledger has no history |
| a live ledger, unmodified | verifies; doctor says so |
| the truncation check removed | the first seed passes silently — the test fails |
