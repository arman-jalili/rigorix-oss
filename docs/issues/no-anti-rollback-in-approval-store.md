---
guardian_issue:
  id: "ISSUE-NO-ANTI-ROLLBACK-IN-APPROVAL-STORE"
  title: "approval: the store authenticates a record's state but not its progress, so restoring a valid older file restores a consumed approval"
  epic: "consequence gating"
  component: "ApprovalBinding"
  module: "approval"
  status: planned
  priority: medium
  tracking_issue: 1241
  dependencies:
    - "#1237 — the authenticated approval store"

  intent: |
    The MAC proves a record was written by this run key. It does not prove the file is the
    current one: an older file, copied while it was authentic, verifies again after the
    records in it have been consumed or expired.

    Concretely: the store is a JSON map keyed by `node_id`, and `consumed`/`expires_at` live
    only inside it. An actor who can write the workspace can keep a copy from before a
    dispatch and restore it afterwards; every MAC in the restored file is valid, the consumed
    record reads `pending`, and `verify_intent` has nothing to compare against but the file.

    The window is narrow — it needs a dispatch that consumed a record and can be retried, for
    example a resume after a crash between consume and execute — but the property the store
    claims ("single-use, time-bound") is the one that a rollback defeats.

    After this change the store can tell that it is being shown an older state: consumption
    is recorded somewhere the workspace cannot rewrite, or the record carries a monotonic
    marker the store refuses to go backwards on.

  in_scope:
    - "A durable record of consumption that a writable store alone cannot roll back"
    - "A store that detects a file whose records regress against that record and refuses, with a diagnosis"
    - "The default store location is documented as inside the writable workspace, alongside RIGORIX_APPROVAL_STORE"

  out_of_scope:
    - "The per-record MAC, which works"
    - "Multi-process locking"
    - "Key distribution"

  affected_layers:
    domain:
      - "Modified: consumption evidence — a marker outside the record file, or a monotonic sequence"
    application:
      - "Modified: verify_intent — a regressed store is an integrity failure, not a match"
    infrastructure:
      - "Modified: repository — writes and checks the marker"
    api:
      - "Modified: doctor reports the store location's writability classification"

  canonical_references:
    - adr: ".pi/architecture/decisions/ADR-011-approval-binding.md"
    - last_sync: "2026-10-31"
    - evidence: "engine/src/approval/infrastructure/repository/approval_repository_impl.rs:164 — the whole map is rewritten on every save"
    - evidence: "engine/src/approval/domain/record.rs — `mac` covers status and expires_at; there is no sequence, counter or anchor"
    - evidence: "engine/src/execution_engine/application/factory.rs:405 — the store path defaults to `.rigorix/approvals.json`, inside the workspace the agent writes"

  acceptance_criteria:
    - "A store restored to a state older than the recorded consumption is refused, naming the step and node"
    - "A store with no marker (fresh, or migrated) still works and says so"
    - "The refusal is seeded: snapshot, consume, restore, dispatch — the dispatch halts"
    - "doctor states whether the store resolves outside the writable workspace"

  validators:
    - ci
    - tests
    - security

  implementation_notes: |
    - This is the residue #1237 did not claim to close: the issue asked for tamper detection
      on the fields and a configurable path, and delivered both. What it did not ask for, and
      what a reviewer will ask next, is what stops a whole-file rollback.
    - The cheapest anchor that fits the existing design is the audit trail: the envelope is
      signed and already records approvals, so a dispatch can check that the consumed record
      has a matching approval event. That ties the store to an artifact the workspace cannot
      rewrite without detection.
    - Do not 'fix' this by hashing the file into itself — a file that vouches for its own
      freshness is the same file an attacker replaces.

  file_changes:
    - "modify: engine/src/approval/infrastructure/repository/approval_repository_impl.rs"
    - "modify: engine/src/approval/application/service_impl.rs"
    - "modify: engine/src/execution_engine/application/factory.rs"
    - "create: engine/tests/approval_store_rollback_integration.rs"

  pipeline_steps:
    - implement
    - validate
    - create-mr
    - merge
---

# ISSUE-NO-ANTI-ROLLBACK-IN-APPROVAL-STORE

## The gap

```
approve  →  file: [node A: pending]        MAC valid
dispatch →  file: [node A: consumed]       MAC valid
restore  →  file: [node A: pending]        MAC valid again
```

Every line authenticates. The MAC answers "did this process write this record", not "is this
the latest state of it". Consumption is recorded only in the file that the actor controls,
so the property "single-use" is enforced against a document the actor can replace with an
earlier copy of itself.

## Why it is not hypothetical

The default store path is `<repo_root>/.rigorix/approvals.json` — inside the tree the agent
writes, which is why #1237 made it configurable. Without an anchor, the configurable path is
the whole mitigation: a deployment that leaves the default has authentication and no
rollback protection, and nothing in the artifact says which of the two it has.

## The required end state

The store can detect that it is being shown a stale state. Either consumption is recorded
somewhere a workspace write cannot reach, or a dispatch cross-checks the consumed record
against signed evidence it already has — the envelope's approval events are the natural
anchor, and they exist.

## How it is proven

| seed | expected |
|---|---|
| snapshot, consume, restore, dispatch | refused, naming the step and node |
| a fresh store with no marker | works, and says the marker is absent |
| a store placed outside the workspace | doctor says so |
| an untouched, current store | dispatches as before |
