---
guardian_issue:
  id: "ISSUE-LOAD-DOES-NOT-CHECK-TERMINAL-RECORDS-AGAINST-THE-LEDGER"
  title: "loading checks the ledger only for pending records, so a terminal record whose marker went missing is loaded without comment"
  epic: "consequence gating"
  component: "ApprovalBinding"
  module: "approval"
  status: planned
  priority: high
  tracking_issue: 1255
  dependencies:
    - "#1241 — the ledger check on load"

  intent: |
    `reject_rolled_back` returns immediately unless the record is `Pending`:

    ```rust
    if !matches!(record.status, ApprovalStatus::Pending) { return Ok(()); }
    if consumed.contains_key(&marker_key(record.node_id, record.nonce)) { … RecordRollback … }
    ```

    The ledger is consulted in one direction only: *this pending record claims to be unspent — is
    there a marker saying otherwise?* The inverse is never asked: *this record says it was spent —
    does the ledger agree?*

    So a store whose terminal records have no markers loads cleanly and reports "N record(s)
    verify", because every record is individually authentic. The ledger is the second witness, and
    nothing compares it to the store on load.

    That matters most when markers go missing through ordinary use rather than attack: #1254 shows
    a second writer dropping them. After that, every load looks healthy while the anti-rollback
    guard has nothing to fire on.

  in_scope:
    - "On load, every terminal record (consumed, expired, superseded) must have a marker; a missing one is a refusal naming the node and step"
    - "The refusal tells the operator what to run: `rigorix approve ledger migrate`, which already compares the two (with `--accept-missing` as the deliberate override)"
    - "The store the deployment does not hold — a fresh host with a carried ledger — is already covered by migration, so the refusal names it rather than a generic integrity error"
    - "Seeds: consumed record with a marker → loads; consumed record without one → refused by node and step; the marker dropped between saves → refused"

  out_of_scope:
    - "Detecting markers that name no store record, which is a report, not a refusal (#1253)"
    - "Rolling the store and the ledger back together, which no local anchor detects"
    - "Making load write anything"

  affected_layers:
    domain:
      - "Modified: the ledger check is bidirectional"
    application:
      - "Modified: approval service load path surfaces the refusal"
    infrastructure:
      - "None — both sets are already in memory at load"

  canonical_references:
    - adr: ".pi/architecture/decisions/ADR-011-approval-binding.md"
    - last_sync: "2026-10-31"
    - evidence: "engine/src/approval/infrastructure/repository/approval_repository_impl.rs — reject_rolled_back returns early for non-pending records"
    - evidence: "measured: a store with one consumed record and a ledger stripped to {} loads as '3 record(s) verify' (doctor), the marker absent and unmentioned"

  acceptance_criteria:
    - "A terminal record with no marker refuses to load, naming node and step"
    - "The refusal names `rigorix approve ledger migrate` and the `--accept-missing` override"
    - "A store whose terminal records are all covered loads unchanged"
    - "Seeds fail if the inverse check is removed"

  validators:
    - ci
    - tests
    - docs

  implementation_notes: |
    - The two checks answer different questions and both are cheap at load: the pending check stops a
      restored approval from being *used*, the terminal check stops marker loss from being
      *invisible*. Only the first exists.
    - Keep migration as the only way to seal an uncovered store. The refusal is what makes an
      operator read the count instead of scrolling past it.
    - `--accept-missing` exists and must be reachable from the load path's message, or a deployment
      that legitimately moved a store without its ledger becomes unstartable with no documented exit.

  file_changes:
    - "modify: engine/src/approval/infrastructure/repository/approval_repository_impl.rs"
    - "modify: engine/src/approval/application/service_impl.rs"
    - "modify: engine/tests/approval_store_rollback_integration.rs"

  pipeline_steps:
    - implement
    - validate
    - create-mr
    - merge
---

# ISSUE-LOAD-DOES-NOT-CHECK-TERMINAL-RECORDS-AGAINST-THE-LEDGER

## The two questions, one of which is asked

```
pending record    + marker        → RecordRollback   (asked today)
terminal record   + no marker     → nothing          (never asked)
```

A terminal record with no marker is the shape #1254 produces, and the shape a marker-dropping bug
produces in any form. Today it is silent: the record verifies by its own MAC, the ledger verifies by
its own MAC, and nobody compares them.

## Measured

```
store:  3 records — 1 consumed, 2 pending
ledger: {}                                  (markers stripped)
$ rigorix-cli doctor
  store …/.rigorix/approvals.json (3 record(s) verify, inside agent-writable workspace)
```

Three records verify. One of them says an approval was spent, and the file that is supposed to
remember that says nothing. Both statements are true in isolation and contradictory together, and
the tool reports the first.

## The required end state

On load, every terminal record has a marker; a missing one refuses with node and step and names
`rigorix approve ledger migrate` (and `--accept-missing`). A covered store loads unchanged.

## How it is proven

| seed | expected |
|---|---|
| consumed record, marker present | loads |
| consumed record, marker absent | refused, naming node and step |
| marker dropped between two saves | refused on the next load |
| migration then load | loads, and the marker is there |
| the inverse check removed | the second seed loads silently — the seed fails |
