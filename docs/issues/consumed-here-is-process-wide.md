---
guardian_issue:
  id: "ISSUE-CONSUMED-HERE-IS-PROCESS-WIDE"
  title: "the refused-consumption discriminator is a process-wide node set, so a second session in the same process is still treated as benign"
  epic: "authority-boundary"
  component: "Approval"
  module: "execution_engine"
  status: planned
  priority: high
  tracking_issue: 1267
  dependencies:
    - "#1263 — the refusal as evidence"
    - "#1261 — live reads"

  intent: |
    #1263 distinguishes two cases: `AlreadyConsumed` is benign when *this executor* consumed the
    approval, and a refusal otherwise. The discriminator is `consumed_here: Mutex<HashSet<Uuid>>` — a
    field on `ExecutionServiceImpl` (`service_impl.rs:576`), initialized once (`:677`) and **never
    cleared**, keyed by `node_id` alone.

    There is exactly one `ExecutionServiceImpl` per process: `build_real_engine` is called by
    `init_host`, which is "idempotent — a second call is a no-op" (`mcp/src/host/mod.rs:1959-1965`).
    Every MCP session in that process shares it, and so shares `consumed_here`.

    The sequence, with the case #1254 and #1261 named as reachable:

    ```
    session A settles node N   -> consume() Ok        -> consumed_here = {N}
    session B settles node N   -> load() sees Consumed (#1262)
                               -> consume() -> AlreadyConsumed
                               -> consumed_here.contains(N) -> BENIGN
                               -> B's node stays completed, no NodeFailed, no event
    ```

    So the fix fires across processes — where a different process has an empty
    `consumed_here` and classifies `ConsumptionReplay`/`AlreadyConsumed` as a refusal — and is
    silent for the two-session deployment the product actually has. The same set also survives into a
    later run that reuses a node id, so a resumed run is benign for the same reason.

    Two smaller faults in the same record: the published `NodeFailed` event hardcodes `attempt: 1`
    (`dispatch.rs`), and the benign decision is made from process memory rather than from the
    artifact that records who consumed.

  in_scope:
    - "`AlreadyConsumed` is benign only when the recorded consumption names the settlement being recorded now — `(dag_id, node_id, attempt)` — not when any session in this process consumed that node"
    - "The ledger marker carries the consuming settlement's identity, so a reader with no process memory can make the same classification"
    - "The `NodeFailed` event carries the real attempt"
    - "Seeds: two sessions in one process settle one node → the second's run carries the refusal and its node is not completed; the same settlement recorded twice → benign; a fresh process → refusal"

  out_of_scope:
    - "The claim/lease protocol (ADR-020 D6), which prevents the double dispatch instead of classifying it"
    - "`NotFound`, which stays benign"
    - "The failure type string, which #1263 decided"

  affected_layers:
    domain:
      - "Modified: ConsumptionMarker carries the consuming settlement identity"
    application:
      - "Modified: consume_on_terminal classifies from the marker, not from a process set"
    infrastructure:
      - "Modified: the ledger write and read carry the settlement identity; the set is removed or scoped and cleared per run"

  citations:
    - file: "engine/src/execution_engine/application/service_impl.rs"
      note: ":576 consumed_here; :677 initialized once; nothing clears it"
    - file: "engine/src/execution_engine/application/service_impl/dispatch.rs"
      note: ":610-626 the benign arm reads consumed_here.contains(&node_id); the NodeFailed event sets attempt: 1"
    - file: "mcp/src/host/mod.rs"
      note: ":1959-1965 init_host is idempotent, so one ExecutionServiceImpl serves every session"
    - file: "engine/src/approval/infrastructure/repository/approval_repository_impl.rs"
      note: "ConsumptionMarker { node_id, nonce, status, recorded_at } — no attempt, no run"

  verification:
    - "two sessions, one process, one node: the second run carries the refusal and its node is not Completed"
    - "the same settlement recorded twice: benign, one marker"
    - "a second process: refusal"
---

# ISSUE-CONSUMED-HERE-IS-PROCESS-WIDE

## The discriminator

```rust
// service_impl.rs:576
consumed_here: Mutex<HashSet<Uuid>>,     // initialized at :677, never cleared

// dispatch.rs:610-626
Err(AlreadyConsumed(_)) if self.consumed_here.lock()
    .map(|c| c.contains(&node_id)).unwrap_or(false) => None,   // benign
Err(e) => refusal
```

`init_host` builds the engine once per process and is documented as idempotent, so every session
shares one service and one set. The set answers "did *somebody in this process* consume this node",
which is not the question. The question is "did *this settlement* consume it".

## Why the shape is right and the source is wrong

Recording the refusal as evidence on the run (#1263) is the correct end state. Reading the answer from
process memory is not: the fact "who consumed this approval" is durable, lives in the ledger, and
outlives the process. A classification that needs process memory to be correct is a classification
that is wrong on restart, wrong in a second session, and unfalsifiable after the fact.

## The required end state

The marker records the consuming settlement — `(dag_id, node_id, attempt)` — and `AlreadyConsumed` is
benign exactly when the marker names the settlement being recorded. A second session in the same
process, and a second process, take the same path and produce the same evidence. The `NodeFailed`
event carries the real attempt.

## How it is proven

| seed | expected |
|---|---|
| two sessions in one process settle one node | the second's run carries the refusal; its node is not Completed |
| the same settlement recorded twice | benign; one marker; one event |
| a separate process settles the same node | refusal, from the marker |
| the process set restored as the discriminator | seed 1 passes silently — the seed fails |
