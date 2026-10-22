---
guardian_issue:
  id: "ISSUE-EXECUTE-NODE-BYPASSES-EFFECT-SEAM"
  title: "execute_node runs the shared gate sequence but not the effect-commitment write-ahead — the #1074 divergence, one layer down"
  epic: "consequence gating"
  epic_id: "EPIC-CONSEQUENCE-GATING"
  tracking_issue: 1185
  component: "ExecutionEngine"
  module: "effect-commitment"
  status: planned
  priority: high
  created_at: "2026-10-23T18:00:00Z"
  updated_at: "2026-10-23T18:00:00Z"

  intent: |
    Put the effect-commitment write-ahead inside the shared gate path, so both execution routes
    fence identically.

    `gate.rs` exists precisely because of #1074: `execute_graph` ran approval binding, precondition,
    sequence policy and capacity; the public `execute_node` ran none of them, and "the difference was
    invisible in the signature" (`gate.rs:1-22`). Both routes now call `gate_node`.

    Effect-commitment was added later and was **not** put in that sequence. Its three calls live around
    the dispatch loop instead:

    ```
    dispatch.rs:143  begin_effect        (parallel path, before spawn)
    dispatch.rs:182  settle_effect       (parallel path, after the result)
    dispatch.rs:308  begin_effect        (loop re-entry)
    dispatch.rs:947,964,973  record_effect_fence
    node.rs          (execute_node_flow) — zero effect-commitment references
    ```

    So `execute_node` applies approval, precondition, sequence and capacity gates, and dispatches
    without a write-ahead record, without a settle, and without a fence. A node taken through that
    route can double-spend exactly as ADR-019 describes, while the same node through `execute_graph`
    cannot.

    **Reachability, stated precisely** (the review that reported this said "reachable from CLI/MCP" —
    that part I could not confirm): no in-repo production caller exists; `execute_node` is a public
    trait method (`service.rs:88`) on a published crate (`rigorix-engine`), and its doc says "used
    internally by the dispatch loop", which is stale — the loop uses its own path in `dispatch.rs`. So
    the hole is latent in-repo and real for any embedder that calls the trait method. That is the same
    shape as #1074: correct on one route, silently different on the other.

  dependencies:
    - name: "gate_node / execute_node_flow"
      type: internal
      note: "gate.rs:1-22 documents the shared sequence; node.rs:10 is the single-node flow."
    - name: "the effect seam calls"
      type: internal
      note: "dispatch.rs:143,182,308,947,964,973 — all parallel-path."
    - name: "ADR-019 / #1074"
      type: external
      note: "The fence is the point of ADR-019; #1074 is the precedent for two routes answering differently."

  in_scope:
    - "Move or duplicate the write-ahead into the shared path so both routes record before dispatch"
    - "Settle in both routes, and record the fence in both"
    - "A test that drives both routes and asserts an identical effect record lifecycle"
    - "Correct the `execute_node` doc comment if it stays unused internally, or route the loop through it"

  out_of_scope:
    - "Changing gate order (approval → precondition → sequence → capacity stays)"
    - "Making execute_node unused/private (a separate API decision)"
    - "The provider key (#1187)"

  affected_layers:
    domain:
      - "None"
    application:
      - "Modified: node.rs (execute_node_flow) or the shared dispatch step"
    infrastructure:
      - "None"
    api:
      - "The public execute_node route gains the same guarantees as execute_graph"

  canonical_references:
    - module: ".pi/architecture/modules/effect-commitment.md"
    - adr:
        - ".pi/architecture/decisions/ADR-019-effect-commitment.md"
        - ".pi/architecture/decisions/ADR-017-consequence-gating.md"
    - last_sync: "2026-10-23"
    - evidence: "gate.rs:1-22 — the #1074 statement: one sequence, both callers"
    - evidence: "dispatch.rs:143,182,308,947,964,973 — the only effect-commitment call sites"
    - evidence: "node.rs — no reference to effect/begin_effect/settle (grep)"
    - evidence: "service.rs:88 — execute_node is a public trait method; no production caller in-repo"

  acceptance_criteria:
    - "A node dispatched through execute_node produces the same record lifecycle as through execute_graph"
    - "Both routes call the write-ahead before the tool is invoked (no tool call without a record)"
    - "Both routes settle and record the fence"
    - "A test drives each route and compares the record states"
    - "The trait doc matches how the method is actually used"
    - "validate-ci.sh, validate-tests.sh, validate-architecture.sh pass"

  validators:
    - architecture-validator
    - ci
    - tests
    - architecture

  implementation_notes: |
    Prefer moving the write-ahead into the shared path over copying it into `node.rs`. The #1074 fix
    worked because the sequence lives in one place; a second copy is the drift `gate.rs` was written to
    prevent, and the copy that gets forgotten is the one that is not in the dispatch loop.

    If the write-ahead must stay in `dispatch.rs` (because it needs the graph), then `execute_node`
    must refuse to dispatch an approval-gated or side-effecting tool until it has a record — the
    explicit alternative is a documented, enforced limitation rather than a silent difference.

    The test is the deliverable: drive both routes with the same tool and assert identical effect
    records. Without it, the two paths will diverge again.

  file_changes:
    - "modify: engine/src/execution_engine/application/service_impl/node.rs"
    - "modify: engine/src/execution_engine/application/service_impl/dispatch.rs (if the step moves)"
    - "create: engine/tests/effect_seam_both_routes.rs"

  pipeline_steps:
    - implement
    - validate
    - create-mr
    - merge
---

# ISSUE-1185: the single-node route has no write-ahead

## What you are looking at

| gate | execute_graph | execute_node |
|---|---|---|
| approval binding | ✅ | ✅ |
| precondition | ✅ | ✅ |
| sequence policy | ✅ | ✅ |
| capacity | ✅ | ✅ |
| **effect write-ahead** | ✅ (dispatch.rs) | **❌** |
| **settle + fence** | ✅ (dispatch.rs) | **❌** |

The four shared gates were unified by #1074. The fifth was added afterwards, outside the shared path.

## In Scope

- The write-ahead, settle and fence on both routes
- A test comparing record lifecycles across routes
- Doc accuracy for `execute_node`

## Out of Scope

- Gate order; API visibility; the provider key (#1187)

## Affected Layers

### Application
- `node.rs` / shared dispatch step

### API
- `execute_node` gains the same guarantees

## Canonical References

- **Module:** `.pi/architecture/modules/effect-commitment.md`
- **ADR-019, ADR-017**
- **Evidence:** `gate.rs:1-22`; the six dispatch.rs call sites; `service.rs:88`

## Acceptance Criteria

| # | Criterion | Validator |
|---|-----------|-----------|
| 1 | Identical record lifecycle on both routes | `validate-tests.sh` |
| 2 | No tool call without a record, either route | `validate-tests.sh` |
| 3 | Settle + fence on both | `validate-tests.sh` |
| 4 | Trait doc matches usage | `validate-canonical.sh` |

## Implementation

1. Read `gate.rs`, `node.rs`, the effect calls in `dispatch.rs`, and the #1074 tests.
2. Move the write-ahead into the shared path, or make `execute_node` refuse without a record.
3. Add the two-route test.

## Pitfalls

- **Do not copy the block into node.rs and stop** — that is the drift the gate module exists to stop.
- **Do not assume it is unreachable because nothing calls it today**; it is public API in a published
  crate.
- **Fix the trait doc** either way: it says the dispatch loop uses it, and it does not.

## Why this is high

It is the exact defect #1074 closed, one layer down: the same node gets a fence on one route and none
on the other, with nothing in the signature to show it.
