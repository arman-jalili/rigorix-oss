---
guardian_issue:
  id: "ISSUE-MIGRATION-ADOPTS-WITHOUT-COMPARING-THE-STORE"
  title: "the migration command adopts a legacy ledger's marker count without comparing it to the store, so a truncated file is adopted as 0 and the operator has no number to check it against"
  epic: "consequence gating"
  component: "ApprovalBinding"
  module: "approval"
  status: planned
  priority: high
  tracking_issue: 1249
  dependencies:
    - "#1247 — explicit migration"

  intent: |
    #1247 made migration explicit and the count visible. It did not give the operator anything
    to check the count against:

    ```
    $ printf '{}' > ~/.rigorix/approval-consumption-<fp>.json
    $ rigorix approve ledger migrate
      adopted 0 marker(s) from the legacy consumption ledger; wrote the sealed form
    ```

    The store beside the ledger is authenticated and holds the records whose consumption the
    ledger is supposed to remember. A legacy map that names fewer terminal records than the store
    does is a truncated map, not an exported one — and the command can say so, because it can
    read both.

    This matters because the refusal message is what sends the operator here: *"unauthenticated
    ledger … run `rigorix approve ledger migrate` to adopt it explicitly"*. An operator following
    that instruction against a file an attacker stripped adopts the emptiness, and the only signal
    is a zero whose wrongness requires already knowing the right number.

  in_scope:
    - "Migration compares the adopted marker set against the store's terminal records (consumed, expired, superseded)"
    - "A marker missing for a terminal record is reported by name and count, and the migration fails without writing the sealed form unless it is passed --accept-missing"
    - "The report prints adopted N of M terminal record(s) in the store"
    - "Seeds: a legacy map with every terminal record → adopted N of N, sealed; a map missing one → refused, naming the node; the same map with --accept-missing → sealed and says what it left unprotected"

  out_of_scope:
    - "Deciding whether a missing marker is ever legitimate — the flag is the operator's answer, and the issue only requires that it be asked"
    - "The MAC and wrapper rules, which work"
    - "Migration in unbound deployments, where the store's records are not authenticated either"

  affected_layers:
    domain:
      - "Modified: migration's report describes its input against the store"
    application:
      - "Modified: the migrate command's preconditions"
    infrastructure:
      - "Modified: ledger migration reads the store"

  canonical_references:
    - adr: ".pi/architecture/decisions/ADR-011-approval-binding.md"
    - last_sync: "2026-10-31"
    - evidence: "engine/src/approval/infrastructure/repository/approval_repository_impl.rs — migrate_consumption_ledger_at reads only the ledger"
    - evidence: "measured: bare map {} → 'adopted 0 marker(s) … wrote the sealed form'; the store held 3 terminal records at the time"

  acceptance_criteria:
    - "Migration reports adopted N of M terminal record(s) in the store"
    - "A terminal record with no marker refuses the migration and names the node and step"
    - "--accept-missing seals the ledger and prints which terminal records it left unprotected"
    - "Seeds fail if the comparison is removed"

  validators:
    - ci
    - tests
    - docs

  implementation_notes: |
    - This is the same rule as the claim audit's: a number is only a check when the reader can
      compare it to the number it should be. "Adopted 0" and "adopted 0 of 0" are different
      facts, and only the second tells an operator whether to worry.
    - The store is already open at this point in the command; the comparison is a set difference
      over `(node_id, nonce)=terminal`.
    - Do not make `--accept-missing` the default and do not prompt. A prompt turns a decision into
      a keystroke, and the flag is what an operator can be asked about later.

  file_changes:
    - "modify: engine/src/approval/infrastructure/repository/approval_repository_impl.rs"
    - "modify: engine/src/execution_engine/application/factory.rs"
    - "modify: cli/src/cli_boundary/dispatch.rs"
    - "modify: engine/tests/approval_store_rollback_integration.rs"

  pipeline_steps:
    - implement
    - validate
    - create-mr
    - merge
---

# ISSUE-MIGRATION-ADOPTS-WITHOUT-COMPARING-THE-STORE

## Measured

```
store:  3 record(s) verify — 1 consumed, 2 pending
ledger: bare map, entries = {}          (a stripped file, and a truncated export look identical)

$ rigorix approve ledger migrate
  adopted 0 marker(s) from the legacy consumption ledger; wrote the sealed form
```

The command did what it was told. The defect is that "0" is the whole report, and the file it
was told about is the one an attacker wrote.

## Why the comparison is available and cheap

The migration runs with the store open — it already reads `approvals.json` to find terminal
records in every other path. `(node_id, nonce)` is the ledger's key and the store's identity, so
"which terminal records have no marker" is a set difference, not an inference.

## The required end state

Migration reports adopted N of M terminal record(s); a terminal record without a marker refuses
the migration and names the node and step; `--accept-missing` seals the ledger and says which
records it left unprotected.

## How it is proven

| seed | expected |
|---|---|
| legacy map with every terminal record | adopted N of N, sealed |
| legacy map missing one terminal record | refused, names the node and step |
| the same map with `--accept-missing` | sealed, prints the unprotected records |
| M terminal records, sealed ledger | already sealed; M marker(s) verify |
| the comparison removed | the second seed passes silently — the test fails |
