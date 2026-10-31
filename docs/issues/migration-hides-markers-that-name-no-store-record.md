---
guardian_issue:
  id: "ISSUE-MIGRATION-HIDES-MARKERS-THAT-NAME-NO-STORE-RECORD"
  title: "migration reports 'adopted 2 of 1 terminal record(s)' and never says how many markers name no record in the store, so a carried ledger and a reset store look alike"
  epic: "consequence gating"
  component: "ApprovalBinding"
  module: "approval"
  status: planned
  priority: medium
  tracking_issue: 1253
  dependencies:
    - "#1251 — the rendered report this extends"

  intent: |
    #1251 made every migration outcome readable. One case it did not cover is reachable on the
    second run of either demo:

    ```
    $ bash reset-demo.sh                       # clears .rigorix/approvals.json, keeps the ledger
    $ rigorix approve ledger migrate
      adopted 2 of 1 terminal record(s) from the legacy consumption ledger; wrote the sealed form
    ```

    `markers` counts every entry in the legacy ledger; `terminal_records` counts the terminal
    records the store holds now. They are different populations, so `N of M` with `N > M` is
    ordinary — and it reads as a contradiction, because the sentence implies N is drawn from M.

    The number that is missing is the one that would explain it: **how many markers name a record
    the store does not hold**. That count distinguishes a ledger carried across a store reset (or
    from another host) from one that matches, and nothing in the report distinguishes them today.
    `missing` covers the opposite direction only — terminal records with no marker.

    So the operator of a migration across hosts gets a report that looks identical to a routine
    one, with a fraction that cannot be true.

  in_scope:
    - "The report states how many markers name no record in the store, and names the two reasons it happens (the store was reset, or the ledger came from another store)"
    - "The counts are stated as populations: markers adopted, terminal records covered, markers with no store record"
    - "`adopted N of M` no longer appears when N > M; the sentence says what was covered instead of implying a fraction"
    - "The already-sealed path reports the same three numbers"
    - "Seeds: 2 markers / 1 terminal record covered → 1 unmatched named; 2 markers / 2 covered → 0 unmatched; 0 terminal records → the #1251 sentence unchanged"

  out_of_scope:
    - "Refusing migration because a marker names no store record — a reset store is a legitimate reason for one"
    - "Deciding whether the store or the ledger is authoritative when they disagree"
    - "The MAC, the wrapper rule, and the missing-marker refusal, which work"

  affected_layers:
    domain:
      - "Modified: the migration outcome counts markers against the store in both directions"
    application:
      - "Modified: the CLI's rendered report"
    infrastructure:
      - "Modified: the store read returns the record identities the markers are compared against"

  canonical_references:
    - adr: ".pi/architecture/decisions/ADR-011-approval-binding.md"
    - last_sync: "2026-10-31"
    - evidence: "measured in authority-demo after reset-demo.sh: `adopted 2 of 1 terminal record(s)` — the ledger held a marker from the previous run's store"
    - evidence: "engine/src/approval/infrastructure/repository/approval_repository_impl.rs — `unprotected_terminal_records` returns (terminal, missing); the marker-side set is never computed"

  acceptance_criteria:
    - "No outcome prints N of M when N > M"
    - "The report names how many markers have no record in the store"
    - "The three numbers are labelled by population, so a reader can tell which is which"
    - "Seeds cover N > M, N == M, and M == 0"
    - "The rendered sentences are asserted, not only the outcome structs"

  validators:
    - ci
    - tests
    - docs

  implementation_notes: |
    - Both demos' reset clears `approvals.json` and keeps the ledger by design (#1241 put the ledger
      outside the workspace), so this is the second-run experience for anyone following the demo.
    - The store read already returns every record; comparing `(node_id, nonce)` against the marker
      keys gives the unmatched set with the same data the missing set uses.
    - Do not fix it by capping N at M. Two markers are genuinely adopted; one simply has nothing to
      cover, and the report should say so.

  file_changes:
    - "modify: engine/src/approval/infrastructure/repository/approval_repository_impl.rs"
    - "modify: engine/src/approval/infrastructure/repository/approval_repository.rs"
    - "modify: cli/src/cli_boundary/dispatch.rs"
    - "modify: cli/tests/doctor_cli.rs"

  pipeline_steps:
    - implement
    - validate
    - create-mr
    - merge
---

# ISSUE-MIGRATION-HIDES-MARKERS-THAT-NAME-NO-STORE-RECORD

## Measured, second run of the demo

```
store:  3 records — 1 consumed, 2 pending
ledger: 2 markers        (one from the previous run's store, which reset-demo.sh removed)

$ rigorix approve ledger migrate
  adopted 2 of 1 terminal record(s) from the legacy consumption ledger; wrote the sealed form
```

Nothing is wrong with the migration. What is wrong is that this sentence and the routine one
(`adopted 1 of 1`) differ only in a fraction that cannot be true, and the reason for the
difference — one marker with no store record — is not in the report.

## Why the missing count matters beyond the demo

A ledger migrated onto a host whose store belongs to a different deployment has exactly this
shape: markers naming records this store has never seen. Today that reports as `N of M` with a
larger N and seals; the operator has no number to look at, and the two situations that produce it
(carried deliberately, or mismatched) are indistinguishable.

## The required end state

The report states three numbers by population — markers adopted, terminal records covered, markers
with no record in the store — and `adopted N of M` never appears when N > M. The already-sealed
path reports the same three.

## How it is proven

| seed | expected |
|---|---|
| 2 markers, 1 terminal record covered | "2 marker(s) adopted: 1 covers a terminal record, 1 has no record in the store" |
| 2 markers, 2 covered | "2 marker(s) adopted: 2 cover terminal records, 0 have no record in the store" |
| 0 terminal records | the #1251 sentence, unchanged |
| 1 terminal record, no marker | refused, naming the node and step |
| any outcome printing `N of M` with N > M | the seed fails |
