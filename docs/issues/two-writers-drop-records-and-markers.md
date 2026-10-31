---
guardian_issue:
  id: "ISSUE-TWO-WRITERS-DROP-RECORDS-AND-MARKERS"
  title: "the approval store and the consumption ledger are cached whole-file rewrites, so a second writer silently drops the first's records and markers"
  epic: "consequence gating"
  component: "ApprovalBinding"
  module: "approval"
  status: planned
  priority: critical
  tracking_issue: 1254
  dependencies:
    - "#1241 — the consumption ledger"
    - "#1244 — the ledger MAC"
    - "#1239 — the record MAC"

  intent: |
    `FileBackedApprovalRepository` loads the store and the ledger once, keeps both in memory, and
    writes the whole file back on every save. Two processes opened on the same paths therefore each
    hold a snapshot, and the second write erases what the first wrote.

    Measured with two repositories opened before either wrote — the normal shape when two MCP
    sessions or an MCP session and a CLI run share a repository:

    ```
    A.save(consumed node A)  → ledger markers = 1, store records = 1
    B.save(consumed node B)  → ledger markers = 1   (A's marker is gone)
                               store records  = 1   (A's record is gone)
    ```

    The consequence is not a lost update in a cache. It is the loss of a consumption marker, which
    is the fact #1241 and #1244 exist to keep: with the marker gone, restoring an older authentic
    copy of the store no longer restores an approval that was already spent, because nothing
    remembers it was spent.

    The store's temp file is a fixed name (`approvals.json.tmp`) while the ledger's is unique, so
    concurrent persists also collide on the rename rather than merely losing a record.

  in_scope:
    - "A write does not erase another writer's records or markers: the file is the source of truth at write time, or an exclusive lock serialises writers"
    - "Both files carry the property: the store and the consumption ledger"
    - "The temp file a writer publishes from is unique per write"
    - "A seed drives two repositories on one store and asserts that both records and both markers survive"

  out_of_scope:
    - "Multi-host coordination — this is one host, two processes, one file"
    - "Changing where the store or the ledger live"
    - "Making every read re-read the file, if a lock gives the same guarantee"

  affected_layers:
    domain:
      - "None — the records and markers are unchanged"
    application:
      - "None"
    infrastructure:
      - "Modified: file-backed persistence — read-modify-write under a lock, or per-record appends"

  canonical_references:
    - adr: ".pi/architecture/decisions/ADR-011-approval-binding.md"
    - last_sync: "2026-10-31"
    - evidence: "engine/src/approval/infrastructure/repository/approval_repository_impl.rs:525 persist() serialises self.records wholesale; :546 save() inserts into the cached map"
    - evidence: "measured with two open repositories: after B saves, A's marker and A's record are both absent from disk"
    - evidence: "mcp/src/host/mod.rs:1633 constructs the store once per session, so a session is a writer for its lifetime"

  acceptance_criteria:
    - "Two writers on one store leave both records and both markers on disk"
    - "The temp path each writer publishes from is unique"
    - "Concurrent saves never return a rename error"
    - "A seed fails if either file is written wholesale from a cached map"

  validators:
    - ci
    - tests
    - security

  implementation_notes: |
    - The ledger already uses a unique temp name, which shows the collision was considered there and
      not for the store; neither file is safe against a lost update.
    - Markers are the more serious half: losing a record makes a step unapprovable and visible;
      losing a marker makes a spent approval look unspent and invisible.
    - An exclusive lock over open-save-close is the smallest change. Re-reading before writing works
      only if the read and the write are atomic together, which is the same lock.

  file_changes:
    - "modify: engine/src/approval/infrastructure/repository/approval_repository_impl.rs"
    - "create: engine/tests/approval_store_two_writers_integration.rs"

  pipeline_steps:
    - implement
    - validate
    - create-mr
    - merge
---

# ISSUE-TWO-WRITERS-DROP-RECORDS-AND-MARKERS

## Measured

```rust
let a = open_with_key_and_ledger(&store, Some(KEY), Some(&ledger))?;   // loads both files
let b = open_with_key_and_ledger(&store, Some(KEY), Some(&ledger))?;   // loads the same, empty

a.save(&consumed(node_a)).await?;   // ledger markers = 1, store records = 1
b.save(&consumed(node_b)).await?;   // ledger markers = 1, store records = 1
```

```
after B: ledger markers = 1
markers: ["<node_b>:<nonce_b>"]
A's node present in ledger: false
store records = 1
A's record in store: false
B's record in store: true
```

## Why the marker loss is the critical half

`reject_rolled_back` refuses a **pending** record whose `(node_id, nonce)` the ledger records as
terminal. If the marker is gone, that guard cannot fire:

| what is lost | visible effect | invisible effect |
|---|---|---|
| a record | the step cannot be approved; the operator sees it | — |
| a marker | nothing | a consumed approval can be restored and dispatched |

So ordinary concurrency produces exactly the state #1241 was filed about, without an attacker.

## Reachability

The MCP host builds the store once per session (`mcp/src/host/mod.rs:1633`), so any session is a
writer for its lifetime. Two sessions on one repository — two agents in one tree, which this
project's own workflow runs — are two writers.

## The required end state

A write never erases another writer's records or markers; both files carry the property; the temp
path is unique per write; a seed drives two writers on one store and asserts that both survive.

## How it is proven

| seed | expected |
|---|---|
| two writers, one record each | both records and both markers on disk |
| two writers, concurrent saves | no rename error, both survive |
| the store written from a cached map | the first seed fails |
| the ledger written from a cached map | the first seed fails |
