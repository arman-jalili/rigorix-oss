---
guardian_issue:
  id: "ISSUE-OPEN-REPOSITORY-NEVER-RE-READS"
  title: "an open approval repository never re-reads the store or the ledger, so a second session can consume an approval the first already consumed"
  epic: "authority-boundary"
  component: "Approval"
  module: "approval"
  status: planned
  priority: critical
  tracking_issue: 1261
  dependencies:
    - "#1254 — the write half (merged, #1257)"
    - "#1241 — the anti-rollback marker"

  intent: |
    #1254 fixed the write half of two writers on one store: every write now re-reads the file
    under an exclusive lock and merges one record or marker. The read half was not addressed, and
    it is the half that guards dispatch.

    Measured with two repositories opened on one store and one ledger, exactly as two MCP sessions
    are (`mcp/src/host/mod.rs` builds the store once per session):

    ```
    B.load before A consumes      -> Ok(Some(Pending))
    A consumes                        markers = 1
    B.load                        -> Ok(Some(Pending))      # stale
    B.save(Consumed)              -> Ok                       # second consumption accepted
    markers after B                    markers = 1           # same key, overwritten
    C (opened after) .load        -> Ok(Some(Consumed))
    ```

    So a session that was open when the approval existed can consume an approval another session has
    already consumed. `reject_rolled_back` does not fire, because it reads the instance's own marker
    map, which was captured at open and never refreshed. The dispatch path is
    `load` → `verify_integrity` → `consume` → `save` (`approval/application/service_impl.rs:184, :222,
    :289`), and every step of it reads the snapshot.

    The inverse case is the same defect seen from the other side: a record written *after* a session
    opened is invisible to it — `load` returns `Ok(None)` for an approval that exists, so the run
    refuses with "no approval" for a record the operator can see on disk.

    The marker is keyed `(node_id, nonce)`, and a second consumption of the same approval has the
    same key by construction, so #1257's merge writes it again with the same value and a new
    `recorded_at`. The ledger therefore cannot show that two consumptions happened: the write that
    makes the store merge-safe also makes the replay look idempotent.

  in_scope:
    - "Read paths re-read the store and the ledger from disk (under a shared lock), so an open instance sees records and markers written by another instance"
    - "A consumption is recorded insert-if-absent, not upsert: a marker that already exists for the same (node_id, nonce) refuses the second consumption, naming node, step and the ledger"
    - "A record that another instance consumed is reported on load as the rollback refusal it is, not as Pending"
    - "Seeds: B opens after A's approval → B.load returns the record; A consumes → B.load refuses; A consumes → B's consume is refused and the marker's recorded_at is unchanged; remove the re-read → the first seed fails"

  out_of_scope:
    - "A file-watching or notification mechanism; a read-time re-read is the whole requirement"
    - "The write path's lock and merge, which #1257 settled"
    - "Making `RecordRollback` say something different; the error already names node and step"

  affected_layers:
    domain:
      - "None: the record and the marker keep their shape"
    application:
      - "Modified: load/consume paths may fail with the rollback refusal they already have"
    infrastructure:
      - "Modified: load, verify_integrity and consumption_marker_count read from disk; record_consumption refuses an existing key"

  citations:
    - file: "engine/src/approval/infrastructure/repository/approval_repository_impl.rs"
      note: ":738 load reads self.records; :757 verify_integrity reads self.records; :770 consumption_marker_count reads self.consumed"
    - file: "engine/src/approval/infrastructure/repository/approval_repository_impl.rs"
      note: ":655 record_consumption re-reads the ledger under the lock but upserts the key"
    - file: "mcp/src/host/mod.rs"
      note: "the approval store is built once per session"

  verification:
    - "two instances, one store, one ledger: both see each other's records on load"
    - "a consumed approval cannot be consumed twice; the second attempt names node, step and ledger"
    - "the ledger holds one marker with the first recorded_at"
---

# ISSUE-OPEN-REPOSITORY-NEVER-RE-READS

## Measured

Two repositories opened on one store and one ledger (one process, two instances — the shape of two
MCP sessions):

```
B.load before A consumes  -> Ok(Some(Pending))
A consumes                -> markers = 1
B.load                    -> Ok(Some(Pending))   # B's snapshot, not the file
B.save(Consumed)          -> Ok                  # accepted
markers after B           -> 1                   # same (node, nonce) key, overwritten
C, opened afterwards      -> Ok(Some(Consumed))  # the file was consistent all along
```

The artifact is consistent; the behaviour is a double dispatch. Nothing in either file records that
two consumptions were written, so the ledger cannot be asked the question after the fact.

## Why this is the read half of #1254 and not a new defect

#1254's statement was "each repository published its own cached snapshot" — two writers erased each
other. The fix made writes re-read. Reads still publish the snapshot: `load`, `verify_integrity` and
`consumption_marker_count` read `self.records` / `self.consumed`, which are refreshed only at `open`
and by this instance's own `update_store`.

The consequence is that the anti-rollback guard is enforced against a *fresh open* and not against a
live second session. That is the case the guard exists for: a process that was running before the
consumption happened.

## The required end state

Reads re-read from disk. `record_consumption` treats an existing marker for the same
`(node_id, nonce)` as a replay and refuses it, rather than rewriting the same key. A record consumed
elsewhere is reported as `RecordRollback` on load.

## How it is proven

| seed | expected |
|---|---|
| B opens after A's approval | B.load returns the record |
| A consumes, then B loads | B.load refuses with the rollback error naming node and step |
| A consumes, then B consumes | refused; the ledger still holds one marker with A's `recorded_at` |
| the re-read removed | seed 1 fails |
| the upsert restored | seed 3 fails above |
