---
guardian_issue:
  id: "ISSUE-PF-ACT-3"
  title: "ACT: end-to-end reachability test (gate armed from config, refusal + evidence)"
  epic: "precondition-followups"
  epic_id: "EPIC-PRECONDITION-FOLLOWUPS"
  component: "ReachabilityTest"
  module: "precondition"
  status: planned
  priority: high
  dependencies:
    - "ISSUE-PF-ACT-2"

  in_scope:
    - "Integration test that builds the executor through the real factory + composition path"
    - "Proves: configured precondition + non-zero check => step refused, tool never called"
    - "Proves: envelope precondition_findings[] records the outcome for the composed run"
    - "Proves: default (no preconditions.toml) => unchanged behavior"

  out_of_scope:
    - "The payouts falsifier demo (separate issue)"
    - "New behavior"

  affected_layers:
    api:
      - "New: engine/tests/precondition_composition_e2e.rs (or mcp/tests)"

  canonical_references:
    - module: "engine/.pi/architecture/modules/consequence-gating.md#acceptance-criteria"
    - code: "engine/src/precondition/application/gate_impl.rs"
    - demo: "engine/.pi/issues/issue-consequence-gating-demo.md"

  acceptance_criteria:
    - "Test builds an executor via ParallelExecutionFactoryImpl with a PreconditionSetup built from a fixture file (NOT a manual with_precondition_gate)"
    - "A matching step with a non-zero check is refused; the tool is never called (spy)"
    - "The signed envelope carries precondition_findings[] with outcome/exit_code/inputs_hash/checked_at"
    - "With no fixture file, behavior is unchanged (dispatch proceeds)"
    - "Test fails on the pre-ACT codebase (regression guard for the dormant-gate gap)"

  validators:
    - ci
    - tests
    - security
    - integration

  implementation_notes: |
    This test is the guard that would have caught the dormant-gate gap. Use a
    real process check (a tiny script exiting 1) rather than a mock, to prove the
    runner + trust-boundary + refuse path together. Keep it hermetic (temp dir).

  file_changes:
    - "create: engine/tests/precondition_composition_e2e.rs"
---

# ISSUE-PF-ACT-3: Reachability test

## Why

Every existing test exercises `PreconditionDispatchGate` in isolation or via a
manual `with_*` call. None proves the **composition** path arms it. Build the
executor the way production does and assert refusal + evidence.

## Assertions

1. Fixture `.rigorix/preconditions.toml` + matching step + check exits 1 → refused, tool not called.
2. Envelope `precondition_findings[0].outcome == "failed"` (or `error`), with exit code + inputs hash + timestamp.
3. No fixture file → dispatch proceeds (fail-open-absent).

## References

- `engine/src/precondition/application/gate_impl.rs`
- `engine/src/execution_engine/application/factory.rs`
