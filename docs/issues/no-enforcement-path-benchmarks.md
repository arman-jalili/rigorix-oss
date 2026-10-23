---
guardian_issue:
  id: "ISSUE-NO-ENFORCEMENT-BENCHMARKS"
  title: "criterion benchmarks the graph, not the gates: no measurement of the enforcement path ADR-017/018/019 added"
  epic: "release hygiene"
  epic_id: "EPIC-OPERATIONAL-USABILITY"
  tracking_issue: 1208
  component: "Engine"
  module: "execution-engine"
  status: planned
  priority: medium
  created_at: "2026-10-24T12:10:00Z"
  updated_at: "2026-10-24T12:10:00Z"

  intent: |
    Measure the path every dispatch takes, not just the graph operations around it.

    The single `[[bench]]` target (`engine/Cargo.toml:66`, `engine/benches/dag_engine.rs`, 60 lines)
    measures `topological_sort` and `seal_graph`. `grep -rn 'gate|approval|precondition|capacity|
    effect_commitment|reserve' engine/benches/` finds nothing.

    Dispatch now passes up to five gates (approval, precondition, sequence, capacity,
    effect-commitment). Three arrived with ADR-017/018/019 and none of them came with a cost. Two can
    leave the process: the precondition runner executes checks, the capacity adapter may be an HTTP
    client, the effect seam may write to a ledger.

    The risk is not that it is slow; it is that nobody would notice if a gate started costing a
    millisecond per node, or a ledger write per attempt. Tests stay green, and the "tiny primitive"
    claim has no number behind it.

  dependencies:
    - name: "criterion"
      type: external
      note: "0.5, already a dev-dependency of engine"
    - name: "dispatch path"
      type: internal
      note: "service_impl/dispatch.rs, application/gate.rs"

  in_scope:
    - "A criterion group on the dispatch path over the DAG shape the graph bench already builds"
    - "Configurations: seam absent (default), seam present with the in-process ledger, capacity declared (stubbed transport), N = 1/10/100"
    - "Record the numbers as a baseline in the module doc or ADR so a regression has something to compare"
    - "No database or network: the HTTP configuration uses a stub, or is ignored in CI"

  out_of_scope:
    - "Micro-benchmarks per gate"
    - "Changing the gate order or semantics"

  affected_layers:
    domain: ["None"]
    application: ["Modified: engine/benches/ (new bench target)"]
    infrastructure: ["None"]
    api: ["None"]

  canonical_references:
    - module: ".pi/architecture/modules/execution-engine.md"
    - adr:
        - ".pi/architecture/decisions/ADR-017-consequence-gating.md"
        - ".pi/architecture/decisions/ADR-018-distributed-capacity-ledger.md"
        - ".pi/architecture/decisions/ADR-019-effect-commitment.md"
    - last_sync: "2026-10-24"
    - evidence: "engine/benches/dag_engine.rs — topological_sort and seal_graph only, 60 lines"
    - evidence: "grep for gate/approval/precondition/capacity/effect in engine/benches/ returns nothing"

  acceptance_criteria:
    - "A bench group exists on the dispatch path with seam-absent and seam-present configurations"
    - "The baseline is recorded where the ADRs can be compared against it"
    - "The bench runs without a database or network"

  validators:
    - tests
    - operations

  implementation_notes: |
    Measure the marginal cost per node, not the absolute runtime of a fixed graph: the question is what
    gating adds, and that is a difference between two configurations on the same DAG.

    Keep the seam configurations explicit rather than defaulted — a bench that silently follows whatever
    `from_env` resolves measures the developer's environment.

    If the HTTP capacity adapter cannot be stubbed cheaply, measure the seam-absent and in-process pair
    first: that answers the cost of the default path, which is what every user pays.

  file_changes:
    - "add: engine/benches/dispatch_gates.rs"
    - "modify: engine/Cargo.toml ([[bench]] target)"
    - "modify: engine/.pi/architecture/modules/execution-engine.md (baseline numbers)"

  pipeline_steps:
    - implement
    - validate
    - create-mr
    - merge
---

# ISSUE-1208: the gates have no cost

## In Scope

- A dispatch-path bench group; four configurations; a recorded baseline; no DB or network

## Out of Scope

- Per-gate micro-benchmarks; changing gate order

## Acceptance Criteria

| # | Criterion | Validator |
|---|-----------|-----------|
| 1 | Dispatch-path bench with seam absent/present | cargo bench |
| 2 | Baseline recorded | doc review |
| 3 | Runs offline | cargo bench |

## Implementation

1. Reuse the DAG builder from `dag_engine.rs`.
2. Add `execute_graph` groups for the four configurations.
3. Record the numbers next to the ADRs' claims.
