---
guardian_issue:
  id: "ISSUE-DEMO-CONSEQUENCE-GATING"
  title: "Falsifier: T0 approve -> dN authority flip -> Tn refuse (demo + E2E)"
  epic: "consequence-gating"
  epic_id: "EPIC-CONSEQUENCE-GATING"
  status: planned
  priority: high
  dependencies:
    - "ADR-017 Phase A — precondition gate + evidence (engine, Guardian-generated)"
    - "ADR-017 Phase D — step-outcome gating (engine, Guardian-generated)"
    - "rigorix-sdk #39 — contract freeze"

  in_scope:
    - "CI-able E2E engine/tests/precondition_e2e.rs (fixture check script; no external deps)"
    - "demo/consequence-gating/run.sh — one command, exit 0 on the four assertions"
    - "A narrative scene (optional) in payouts-demo mirroring the money-out story"
    - "Prints the signed envelope precondition_findings[] as the preserved result"

  out_of_scope:
    - "New engine behavior (this issue only demonstrates)"

  canonical_references:
    - module: ".pi/architecture/modules/precondition.md"
    - adr: ".pi/architecture/decisions/ADR-017-consequence-gating.md"
    - demo: "demo/anchor-e2e/run.sh (pattern for a one-command harness)"

  acceptance_criteria:
    - "Assertion 1: with the authority standing, the run dispatches (check exits 0)"
    - "Assertion 2: after the authority flips (check exits non-zero), the run REFUSES and the tool is never called"
    - "Assertion 3: the refusal is recorded in the signed envelope with outcome+exit_code+inputs_hash+timestamp"
    - "Assertion 4: with no preconditions configured, behavior is unchanged (fail-open-absent)"
    - "demo/consequence-gating/run.sh exits 0 with all four assertions green"
    - "The demo output is presentation-ready (the falsifier Tim asked to see)"

  validators:
    - ci
    - tests
    - integration

  implementation_notes: |
    Keep the check domain-neutral: a small script that reads the JSON on stdin
    and exits 0/non-zero based on a fixture file (the "authority"). The flip is a
    file edit between the approve step and the execute step. Do not use a model.
    Reuse the approval pause/approve/resume path to make T0 explicit. The signed
    envelope is the artifact to show; print the precondition finding.

  file_changes:
    - "create: engine/tests/precondition_e2e.rs"
    - "create: demo/consequence-gating/run.sh"
    - "create: demo/consequence-gating/check-authority.sh"
    - "create: demo/consequence-gating/README.md"
    - "create: demo/consequence-gating/.rigorix/preconditions.toml"
---

# ISSUE-PG-09: The falsifier

## Intent

Produce the artifact that answers Tim's challenge with evidence: a run that is
approved at T₀, whose authority changes at ΔN, and which **refuses at Tₙ** —
with the determination preserved in the signed record.

## Scene

```
T0  approve the payout (human)
    |
dN  flip the beneficiary to ineligible (authority fixture changes)
    |
Tn  dispatch the payout step
      -> check exits non-zero
      -> run refuses; the payout tool is never called
      -> signed envelope carries precondition_findings: {outcome:"failed", exit_code:3, inputs_hash, checked_at}
```

## Assertions (the four the report promised)

1. authority standing -> dispatch
2. authority flipped -> refuse, tool never called
3. refusal in the signed evidence
4. no preconditions configured -> unchanged (fail-open-absent)

## References

- `demo/anchor-e2e/run.sh` (one-command harness pattern)
- `engine/src/execution_engine/approval_binding_tests.rs` (approval pause/approve/resume harness)
