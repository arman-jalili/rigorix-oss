---
guardian_issue:
  id: "ISSUE-BENCH-BASELINE-NOT-REPRODUCIBLE"
  title: "the recorded enforcement-path baseline does not reproduce: the 26% seam cost re-measures as 3% inside overlapping CIs, and the doc claims 50 samples the bench never sets"
  epic: "release hygiene"
  epic_id: "EPIC-OPERATIONAL-USABILITY"
  tracking_issue: 1212
  component: "Engine"
  module: "execution-engine"
  status: planned
  priority: medium
  created_at: "2026-10-24T14:40:00Z"
  updated_at: "2026-10-24T14:40:00Z"

  intent: |
    Make the published baseline either reproduce or stop being a table of absolutes.

    The bench is well built and should not change: explicit seam configurations, a fresh `dag_id` per
    iteration, concurrency pinned to 1, `black_box` on the output, and HTTP capacity deliberately left
    out with the reason given. What it produced does not survive re-measurement on the same machine:

    | cell | doc | re-measured |
    |---|---|---|
    | 1 node, absent | 166 µs [162-174] | 198 µs [171-233] |
    | 1 node, present | 193 µs [179-210] | 205 µs [187-228] |
    | 100 nodes, absent | 19.8 ms [17.8-22.3] | 16.8 ms [15.7-18.4] |
    | 100 nodes, present | 25.0 ms [21.8-28.8] | 17.3 ms [16.5-18.4] |

    The documented conclusion — "the in-memory seam adds a measurable but single-digit-ms cost at 100
    nodes" — is +5.2 ms / +26% with separated CIs as recorded, and +0.5 ms / +3% with overlapping bands
    as measured. The 1-node pair no longer separates either.

    Two supporting defects: the doc says "criterion, 50 samples" while the bench sets no `sample_size`
    (criterion default is 100), and `--quick` returns 43.6 µs for a cell that takes 198 µs by default —
    a 4.5x error, larger than the effect being measured.

  dependencies:
    - name: "bench"
      type: internal
      note: "engine/benches/dag_engine.rs::bench_enforcement_path"
    - name: "recorded baseline"
      type: internal
      note: "engine/.pi/architecture/modules/execution-engine.md:276-291"

  in_scope:
    - "Either re-derive the table under stated conditions (idle machine, same commit, no --quick) with the machine and flags recorded, or drop the absolute numbers and keep only the before/after method"
    - "Set sample_size explicitly if 50 is intended, otherwise correct the text"
    - "Derive the per-node floor from the 100-node slope, not the 1-node cell (60 µs band at N=1)"
    - "State whether the seam cost is below the harness's noise floor for N <= 100"

  out_of_scope:
    - "Changing the bench structure, configurations, or the HTTP omission"
    - "Adding a CI perf gate (a separate decision; criterion in CI needs a stored baseline and a runner)"

  affected_layers:
    domain: ["None"]
    application: ["None"]
    infrastructure: ["Modified: engine/.pi/architecture/modules/execution-engine.md; possibly engine/benches/dag_engine.rs (sample_size)"]
    api: ["None"]

  canonical_references:
    - module: ".pi/architecture/modules/execution-engine.md"
    - adr:
        - ".pi/architecture/decisions/ADR-019-effect-commitment.md"
    - last_sync: "2026-10-24"
    - evidence: "re-measured 1 node absent 198us [171-233], present 205us [187-228] (doc: 166 [162-174], 193 [179-210])"
    - evidence: "re-measured 100 nodes absent 16.8ms [15.7-18.4], present 17.3ms [16.5-18.4] (doc: 19.8 [17.8-22.3], 25.0 [21.8-28.8])"
    - evidence: "grep sample_size engine/benches/dag_engine.rs -> none; criterion default 100"
    - evidence: "--quick on seam_absent/1_nodes -> 43.6us vs 198us by default"

  acceptance_criteria:
    - "Every number in the table is either reproduced with recorded conditions, or removed"
    - "The sample count in the text matches the code"
    - "The per-node floor claim cites the 100-node slope"
    - "The seam-cost conclusion is stated only as strongly as the CIs support"

  validators:
    - operations
    - tests

  implementation_notes: |
    Prefer the method over the table. The doc already says the right thing - "run before and after on the
    same tree" - and that is what the group's stored criterion baseline gives you. Absolute numbers from
    a laptop are a claim about the laptop as much as about the code, and the reader cannot verify which.

    If the table stays, it needs the conditions that produced it: machine, load, commit, and the exact
    criterion flags. That is what makes it reproducible rather than an anecdote.

    The honest strengthening: if the seam's cost is inside the noise band at N <= 100, that is a *better*
    result than a 26% delta, and it can be stated as measured once the band is honest.

  file_changes:
    - "modify: engine/.pi/architecture/modules/execution-engine.md (baseline section)"
    - "modify: engine/benches/dag_engine.rs (only if sample_size is set)"

  pipeline_steps:
    - implement
    - validate
    - create-mr
    - merge
---

# ISSUE-1212: a baseline that does not reproduce

## In Scope

- Re-derive under stated conditions or drop the absolutes; sample count matches code; floor from the
  100-node slope; state the noise floor

## Out of Scope

- Bench structure/configurations; a CI perf gate

## Acceptance Criteria

| # | Criterion | Validator |
|---|-----------|-----------|
| 1 | Table numbers reproduced with conditions, or removed | re-run |
| 2 | Sample count matches `sample_size` | review |
| 3 | Floor cites the 100-node slope | review |
| 4 | Seam claim bounded by the CIs | review |

## Implementation

1. Decide: table-with-conditions, or method-only.
2. Set `sample_size(50)` if that is the intent; otherwise fix the text.
3. Re-run `cargo bench --bench dag_engine -- enforcement_path/{seam_absent,seam_in_memory}/{1,10,100}_nodes` on an idle machine and replace or delete the table.
4. Restate the conclusion from the CI bands.
