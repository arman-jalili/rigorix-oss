---
guardian_issue:
  id: "ISSUE-EXECUTE-NODE-BYPASSES-CONSEQUENCE-GATES"
  title: "execution-engine: the public execute_node trait method runs a node without the approval, precondition, sequence or capacity gates"
  epic: "execution boundary"
  component: "ParallelExecutionService"
  module: "execution-engine"
  status: planned
  priority: high
  dependencies:
    - "ADR-017 — consequence gating at dispatch"

  intent: |
    Give the engine one governed execution entry point, or make the ungoverned one impossible to
    mistake for it.

    `run_dispatch_loop` applies the gates: intent verification, precondition evaluation,
    sequence policy, capacity reservation. `execute_node` on the public `ExecutionEngine` trait
    calls `execute_node_flow`, which calls `execute_tool` directly — it applies permission gating
    (including the bash check) and nothing else.

    So a library consumer that calls `execute_node` gets permission checks and no consequence
    gating, while the same tool invoked through a graph gets both. The difference is one trait
    method and is not visible in the type.

  in_scope:
    - "Decide the contract for execute_node: governed, or explicitly low-level"
    - "If governed, route it through the same gate sequence as the dispatch loop"
    - "If low-level, rename/document it as ungoverned and keep it off the public governed trait"
    - "Make the distinction visible in the type, not only in a doc comment"
    - "Test: the same tool through both routes produces the same finding for a denied precondition"

  out_of_scope:
    - "Exposing a single-node governed method over MCP (that is a frozen-catalog change)"
    - "Removing the dispatch loop's concurrency"

  affected_layers:
    domain:
      - "Modified: ExecutionEngine trait — governed vs low-level execution"
    application:
      - "Modified: execute_node / execute_node_flow — gate sequence or explicit contract"
    infrastructure:
      - "None"
    api:
      - "Modified: trait surface (and any SDK mirror of it)"

  canonical_references:
    - adr: ".pi/architecture/decisions/ADR-017-consequence-gating.md"
    - evidence: "engine/src/execution_engine/application/service_impl.rs:873-878 — async fn execute_node calls execute_node_flow and is part of the public trait"
    - evidence: "engine/src/execution_engine/application/service_impl/node.rs:7,98 — execute_node_flow calls self.execute_tool directly"
    - evidence: "engine/src/execution_engine/application/service_impl/dispatch.rs — precondition, sequence, approval and capacity gates live here only"
    - evidence: "grep execute_node across mcp/ server/ cli/ actions/ — no exposure, so this is a library-surface contract gap, not a live MCP route"

  acceptance_criteria:
    - "Calling execute_node on a node whose precondition denies does not execute the tool"
    - "The governed/ungoverned distinction is nameable by a caller without reading source"
    - "A test proves both routes agree on a denied node"
    - "The module doc states which routes are governed"
    - "No public method silently skips the gates"

  validators:
    - ci
    - tests
    - architecture

  implementation_notes: |
    - Not currently exploitable over MCP — the method is not exposed — so this is a contract and
      future-integration issue, not an active vulnerability. Say that in the issue status so it is
      not over-prioritised; the priority is high because a library consumer is the intended user
      and the trap is invisible.
    - The cheapest correct move is a single choke point: extract the gate sequence into one
      function both routes call. Duplicating the gates in execute_node will drift.
    - If the low-level route must stay for testing, put it behind a name that says so
      (`execute_node_ungoverned`) and keep it off the default trait.
    - Check whether the SDK mirrors this trait; if so, the rename is a cross-repo contract change
      and follows the catalog-first rule.

  file_changes:
    - "modify: engine/src/execution_engine/application/service_impl.rs"
    - "modify: engine/src/execution_engine/application/service_impl/node.rs"
    - "modify: engine/src/execution_engine/application/factory.rs"
    - "create: engine/tests/execute_node_governed_test.rs"
    - "modify: engine/.pi/architecture/modules/execution-engine.md"

  pipeline_steps:
    - implement
    - validate
    - create-mr
    - merge
---

# ISSUE-EXECUTE-NODE-BYPASSES-CONSEQUENCE-GATES

## Two routes, different guarantees

```rust
// service_impl.rs:873 — public trait method
async fn execute_node(&self, input: ExecuteNodeInput) -> Result<ExecuteNodeOutput, ExecutionError> {
    self.execute_node_flow(input).await
}

// node.rs:7,98
pub(super) async fn execute_node_flow(...) {
    ...
    let task_result = self.execute_tool(&node, node_id, start).await;
}
```

| route | permission | approval binding | precondition | sequence | capacity |
|---|---|---|---|---|---|
| `execute_graph` → `run_dispatch_loop` | yes | yes | yes | yes | yes |
| `execute_node` → `execute_node_flow` | yes | **no** | **no** | **no** | **no** |

The same tool, the same plan, two answers depending on which method the caller used — and the
difference is not visible in the signature.

## Scope of the exposure

The method is **not** exposed by MCP, the server, the CLI or the Action (`grep execute_node`
outside the engine returns nothing), so there is no live remote route. This is a public
**library** contract gap: the engine is a crate, and `execute_node` is on its main trait.

The agent that found this was appropriately careful not to call it exploitable. It is still a
trap: a consumer who builds on the engine and calls the obvious single-node method gets no
consequence gating, and nothing tells them.

## Suggested fix

One choke point, not two gate sequences:

1. Extract the gate sequence from `run_dispatch_loop` into a function that takes one node.
2. Have both routes call it.
3. If a deliberately ungoverned route is needed for tests, name it
   `execute_node_ungoverned` and keep it off the default trait.

Duplicating the gates in `execute_node` will drift within a release or two — the capacity seam
alone touches two call sites today.
