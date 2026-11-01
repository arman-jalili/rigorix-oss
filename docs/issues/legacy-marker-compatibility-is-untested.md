---
guardian_issue:
  id: "ISSUE-LEGACY-MARKER-COMPATIBILITY-IS-UNTESTED"
  title: "the claim that a ledger written before the marker recorded its settlement still verifies has no test in the tree"
  epic: "authority-boundary"
  component: "Approval"
  module: "approval"
  status: planned
  priority: medium
  tracking_issue: 1269
  dependencies:
    - "#1267 — the marker gained dag_id and attempt"

  intent: |
    #1268 added `dag_id` and `attempt` to `ConsumptionMarker` as `Option` fields with
    `skip_serializing_if = "Option::is_none"`, and kept `CONSUMPTION_LEDGER_VERSION` at `1`, so a
    marker written before the change serializes to the same bytes and the whole-set MAC still covers
    it. That is a claim about an **on-disk format that ships to customers** and it has no test: the
    suite exercises the new marker (`the_consumption_marker_records_the_settling_run_and_attempt`) and
    the classification (`another_settlement_is_refused_from_the_marker_not_a_process_set`), but nothing
    constructs a pre-change marker.

    Measured with a throwaway probe (since deleted) that re-seals a ledger in the pre-#1268 shape —
    four fields, declaration order, MAC over `(1u8, entries)`:

    ```
    legacy ledger loads:        Ok(Some(Consumed))
    its settlement is unknown:  Ok(None)
    re-consume classifies as:   Err(ConsumptionReplay …)
    ```

    So the behaviour is right today, and it is unenforced. The failure mode if it drifts is the worst
    kind: not a test going red here, but every existing ledger becoming unreadable at a customer's
    site after an upgrade — `ConsumptionLedgerIntegrity`, which reads exactly like tampering.

  in_scope:
    - "A byte-level fixture writes a pre-#1268 ledger (four-field markers, MAC over `(1, entries)`) beside a store with the terminal record, and asserts: the ledger loads, `consumption_settlement` is `None`, and a consumption is refused"
    - "The fixture is added to `engine/tests/approval_store_rollback_integration.rs`, not to a private probe"
    - "A second seed: two markers for one node with different nonces, where the older one is read for the record that is now current, keeps classifying against the marker for the current nonce"

  out_of_scope:
    - "A migration of the ledger; the format is unchanged"
    - "The classification rule, which #1267 settled"
    - "Byte-for-byte stability of `recorded_at`, which the fixture pins by construction"

  affected_layers:
    domain:
      - "None"
    application:
      - "None"
    infrastructure:
      - "Modified: tests only"

  citations:
    - file: "engine/src/approval/infrastructure/repository/approval_repository_impl.rs"
      note: ":32-44 the marker with skipped optional fields; :130 the version stays 1; :132-142 the MAC covers (version, entries)"
    - file: "engine/tests/approval_live_reads_integration.rs"
      note: ":127 the new-marker seed — the only marker-shape test"

  verification:
    - "remove `skip_serializing_if` from one new field and the legacy fixture fails with a MAC mismatch"
    - "the fixture passes on the current code"
---

# ISSUE-LEGACY-MARKER-COMPATIBILITY-IS-UNTESTED

## What is claimed and what proves it

| claim | where it is stated | what tests it |
|---|---|---|
| a marker records `(dag_id, node_id, attempt)` | `record_consumption`, `ConsumptionSettlement` | `the_consumption_marker_records_the_settling_run_and_attempt` |
| a legacy marker classifies as another settlement, fail-safe | the `Option` fields and `consumption_settlement` | nothing |
| a legacy marker keeps its MAC bytes | `skip_serializing_if` + unchanged `CONSUMPTION_LEDGER_VERSION` | nothing |

The third row is the one that ships in a customer's `$HOME` today. It is also the row a future edit
is most likely to break without noticing: adding one non-skipped field, or bumping the version
constant, changes the material for *every* marker at once.

## Measured (probe, deleted after use)

```
legacy ledger loads:        Ok(Some(Consumed))
its settlement is unknown:  Ok(None)
re-consume classifies as:   Err(ConsumptionReplay …)
```

The behaviour is correct. The evidence should be in the suite, not in a transcript.

## The required end state

A fixture in `approval_store_rollback_integration.rs` writes the pre-change ledger shape, seals it with
the deployment key, and asserts all three lines above. Removing the compatibility — a bumped version,
a non-skipped field — turns it red here instead of at a customer's site.
