---
guardian_issue:
  id: "ISSUE-MIGRATION-REPORTS-ADOPTED-OF-ZERO"
  title: "migration prints 'adopted 1 of 0 terminal record(s)' when the store holds nothing to compare, and cannot say whether the store was absent or empty"
  epic: "consequence gating"
  component: "ApprovalBinding"
  module: "approval"
  status: planned
  priority: medium
  tracking_issue: 1251
  dependencies:
    - "#1249 — the comparison this reports on"

  intent: |
    #1249 made migration compare its marker set with the authenticated store, and report
    `adopted N of M terminal record(s)`. With no terminal records in the store the sentence is
    not readable:

    ```
    $ mv .rigorix/approvals.json /tmp/store-hidden        # a fresh clone, or a reset tree
    $ rigorix approve ledger migrate
      adopted 1 of 0 terminal record(s) from the legacy consumption ledger; wrote the sealed form
    ```

    `1 of 0` is not a fraction a reader can act on, and the fact it hides is the one that matters:
    **nothing was compared**. A ledger carried over from another host, a store cleared by
    `reset-demo.sh`, and a store whose terminal records were all pruned all render the same way.

    Two different absences are also collapsed into one zero: a store that is not there, and a
    store that is there with no terminal records. The first means the comparison was impossible;
    the second means it ran and found nothing to cover.

    The unit test for this path asserts `terminal_records: 0` and never reads the sentence, which
    is why a number that cannot be read passed a test of the number.

  in_scope:
    - "The migration outcome distinguishes 'store absent' from 'store present with no terminal records'"
    - "With no terminal records to compare, the report says the migration compared nothing rather than printing `N of 0`"
    - "With a store absent, the report names the path it did not find"
    - "A test asserts the rendered sentence for each of: store absent, store with no terminal records, store complete, store incomplete"

  out_of_scope:
    - "Refusing to migrate when the store is absent — a ledger carried to a new host is legitimate"
    - "Rolling the store and the ledger back together, which no local anchor detects"
    - "The comparison itself, which works"

  affected_layers:
    domain:
      - "Modified: `LedgerMigration` carries whether the store was read"
    application:
      - "Modified: the CLI's report for each migration outcome"
    infrastructure:
      - "None — the store read already distinguishes the two cases"

  canonical_references:
    - adr: ".pi/architecture/decisions/ADR-011-approval-binding.md"
    - last_sync: "2026-10-31"
    - evidence: "engine/src/approval/infrastructure/repository/approval_repository_impl.rs — unprotected_terminal_records returns (0, []) for both NotFound and an empty store"
    - evidence: "cli/src/cli_boundary/dispatch.rs — `adopted {markers} of {terminal_records} terminal record(s)`"
    - evidence: "measured: store moved aside, legacy ledger with 1 marker → `adopted 1 of 0 terminal record(s)`"

  acceptance_criteria:
    - "No outcome renders `N of 0`"
    - "Store absent: the report says the store was not found at its path, so nothing was compared"
    - "Store present, no terminal records: the report says the comparison found no terminal records to cover"
    - "Store present with terminal records: `adopted N of M` unchanged, and `--accept-missing` still lists what stays unprotected"
    - "The four rendered sentences are asserted by tests, not only the outcome structs"

  validators:
    - ci
    - tests
    - docs

  implementation_notes: |
    - The struct already carries `terminal_records: usize`; the missing fact is whether the store
      was read at all, which the reader of `unprotected_terminal_records` already knows. Carrying
      it is cheaper than inferring it downstream.
    - Asserting the struct is what let this through: the sentence is the product, and this repo
      has a rule for it — a number is only a check when the reader can compare it to the number it
      should be. `1 of 0` compares to nothing.
    - Do not drop the count from the message to dodge the phrasing. `adopted 1 marker(s)` alone is
      the pre-#1249 sentence, and it was filed as the defect.

  file_changes:
    - "modify: engine/src/approval/infrastructure/repository/approval_repository_impl.rs"
    - "modify: engine/src/approval/infrastructure/repository/approval_repository.rs"
    - "modify: cli/src/cli_boundary/dispatch.rs"
    - "modify: engine/tests/approval_store_rollback_integration.rs"
    - "modify: cli/tests/doctor_cli.rs"

  pipeline_steps:
    - implement
    - validate
    - create-mr
    - merge
---

# ISSUE-MIGRATION-REPORTS-ADOPTED-OF-ZERO

## Measured

```
store:  3 records (1 consumed, 2 pending)        → adopted 1 of 1 terminal record(s)
store:  moved aside (NotFound)                   → adopted 1 of 0 terminal record(s)
store:  present, no terminal records             → adopted N of 0 terminal record(s)
```

Same ledger, same key, three different facts about how much was checked, two of them rendered the
same way and neither of them readable.

## Why it is not only cosmetic

The number exists to answer "did the ledger cover what the store knows was spent?". With `M = 0`
the answer is "nothing was asked", and `1 of 0` reads like a successful comparison of one thing
against zero. An operator migrating a ledger onto a host whose store is empty is exactly the person
who needs to know the check did not run.

A store that is *absent* is also a different situation from a store with nothing terminal in it:
the first is a migration across hosts, the second is a young deployment.

## The required end state

No outcome renders `N of 0`. Store absent → the report names the path it did not find. Store
present with no terminal records → the report says the comparison had nothing to cover. Store with
terminal records → `adopted N of M` unchanged. Each sentence is asserted.

## How it is proven

| seed | expected |
|---|---|
| store absent, ledger with 1 marker | "adopted 1 marker(s); the store was not found at `<path>`, so nothing was compared" |
| store present, no terminal records | "adopted 1 marker(s); the store holds no terminal record(s), so nothing was compared" |
| store with 1 terminal, complete | `adopted 1 of 1 terminal record(s)` |
| store with 1 terminal, incomplete | refused, naming the node and step |
| any outcome rendering `of 0` | the seed fails |
