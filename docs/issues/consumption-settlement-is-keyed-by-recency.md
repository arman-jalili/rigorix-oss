---
guardian_issue:
  id: "ISSUE-CONSUMPTION-SETTLEMENT-IS-KEYED-BY-RECENCY"
  title: "consumption_settlement picks the marker with the latest recorded_at, not the current approval's nonce, so the seed that says it checks the nonce passes for the wrong reason"
  epic: "authority-boundary"
  component: "Approval"
  module: "approval"
  status: planned
  priority: medium
  tracking_issue: 1271
  dependencies:
    - "#1267 — markers carry the settlement"
    - "#1269 — the fixture this issue extends"

  intent: |
    The classifier asks the ledger "who consumed this node": `consumption_settlement(node_id)` filters
    markers by `node_id` and takes `max_by_key(recorded_at)` — **recency**, not the approval's identity.
    A node can carry more than one marker, because a re-approval gives the record a new nonce and the
    old marker stays.

    Measured (probe, since deleted) with two settlements for one node: the current approval's nonce
    recorded first, an older approval's nonce for the same node recorded afterwards — the ordering a
    second host with a clock ahead, a restored ledger, or an out-of-order replay produces:

    ```
    asked for node <n>   current approval nonce = …0002
    old     settlement dag = …000b          (recorded last)
    current settlement dag = …0016
    returned               = Some(…000b)     <- the OLD settlement
    keyed by nonce?        = false
    ```

    In the dispatch path that answer is compared with the settlement being recorded. When it comes
    back wrong, a legitimate idempotent settlement is classified as *another settlement*: the node is
    marked failed and a `NodeFailed` event is published for a run that did nothing wrong (#1263's
    machinery, pointed at a false positive).

    The test added by #1270, `the_current_nonce_wins_over_an_older_marker_for_the_same_node`, asserts
    that the classification uses the marker for the current nonce — and passes because the current
    marker is written **last**, which is the same rule the implementation uses. Order the writes the
    other way and it returns the older settlement. The property is asserted, not implemented.

  in_scope:
    - "`consumption_settlement` takes the approval's identity — `(node_id, nonce)` — and returns only the marker for that nonce; the caller has the record it just tried to settle"
    - "The seed constructs the older marker with a **later** `recorded_at` and still expects the current nonce's settlement, so it fails on recency"
    - "Equal `recorded_at` values are not a tie-break: the answer must not depend on insertion order"
    - "A settlement recorded for a nonce the store no longer holds resolves to `None`, which classifies as another settlement — fail-safe, as today"

  out_of_scope:
    - "The pre-#1268 fixture, which is right"
    - "The classification rule in `consume_on_terminal`, which is right"
    - "Pruning old markers"

  affected_layers:
    domain:
      - "None"
    application:
      - "Modified: the port signature and its caller"
    infrastructure:
      - "Modified: the lookup keys by the marker key, which already encodes the nonce"

  citations:
    - file: "engine/src/approval/infrastructure/repository/approval_repository_impl.rs"
      note: ":847-868 consumption_settlement — filter by node_id, max_by_key(recorded_at)"
    - file: "engine/tests/approval_store_rollback_integration.rs"
      note: "the_current_nonce_wins_over_an_older_marker_for_the_same_node — the current marker is written last"
    - file: "engine/src/execution_engine/application/service_impl/dispatch.rs"
      note: "the refusal path compares the returned settlement with the one being recorded"

  verification:
    - "older marker written last: the current nonce's settlement is still returned"
    - "a nonce the store no longer holds: None"
    - "the lookup by node_id alone restored: the first seed fails"
---

# ISSUE-CONSUMPTION-SETTLEMENT-IS-KEYED-BY-RECENCY

## Measured

Two settlements for one node. The current approval's nonce is recorded first, an older approval's nonce
for the same node afterwards:

```
asked for node <n>   current approval nonce = 00000000-…-0002
old     settlement dag = 00000000-…-000b    (recorded last)
current settlement dag = 00000000-…-0016
returned               = Some(00000000-…-000b)
keyed by nonce?        = false
```

## Why it matters

`mac` and the settlement exist so the dispatch layer can tell "this settlement already recorded its
consumption" (benign) from "another settlement consumed this approval" (a refusal that fails the node
and publishes evidence). With recency as the key, the benign case can be read as the refusal case, and
the failure is manufactured: `NodeFailed` on a node whose consumption was recorded by exactly the
settlement that is asking.

## Why the new test does not catch it

`the_current_nonce_wins_over_an_older_marker_for_the_same_node` writes the current marker after the
older one, so `max_by_key(recorded_at)` returns it and the assertion holds — for the ordering, not for
the identity. That is the pattern this repository has already named twice: a test that passes for the
wrong reason (`#466`'s nanosecond note). A seed must produce the ordering that would break a
time-keyed lookup, or it is testing the clock.

## The required end state

The lookup takes `(node_id, nonce)` and returns that nonce's marker or `None`. The seed records the
older nonce **last** and still expects the current one.
