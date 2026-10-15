---
guardian_issue:
  id: "ISSUE-CAPACITY-LEDGER-SEAM"
  title: "capacity_ledger: wire reserve/commit/release into the enforcement path"
  epic: "distributed capacity ledger"
  epic_id: "EPIC-CAPACITY-LEDGER"
  component: "Enforcement"
  module: "capacity-ledger"
  status: planned
  priority: high
  dependencies:
    - "ISSUE-CAPACITY-LEDGER-INPROC"
    - "ISSUE-CAPACITY-LEDGER-HTTP"
    - "ADR-018"

  intent: |
    Make the ledger actually govern execution. Before permitting a governed step, reserve; on
    Insufficient, refuse with a recorded reason; after the effect, commit; on failure or drop,
    release. Reuse the ADR-008 RAII discipline. This is the issue where "implemented but never
    called" is prevented — and it is verified by a reachability test.

  in_scope:
    - "Modified: the enforcement/dispatch seam performs reserve before allowing"
    - "New: RAII guard committing on success and releasing on failure/drop"
    - "Refusal on Insufficient is recorded (attribution reason), and is not retryable"
    - "Fail-closed: Unavailable also refuses"
    - "Only resources declared ledger-governed are affected; everything else is unchanged"
    - "Reachability test: the gate calls the port on the real execution path"

  out_of_scope:
    - "The declaration/configuration TOML surface (additive follow-up)"
    - "Replacing the existing LLM budget (deliberately staged)"
    - "Changing sequence_policy behaviour"

  affected_layers:
    domain:
      - "Modified: refusal reason for capacity exhaustion (attribution)"
    application:
      - "Modified: enforcement seam — reserve/commit/release around the effect"
      - "New: RAII guard binding the grant to the step lifetime"
    infrastructure:
      - "Modified: composition root injects the configured adapter; default is in-process"
    api:
      - "Modified: surfaces render the capacity refusal distinctly from a policy refusal"

  canonical_references:
    - module: ".pi/architecture/modules/capacity-ledger.md"
    - adr:
        - ".pi/architecture/decisions/ADR-018-distributed-capacity-ledger.md"
        - ".pi/architecture/decisions/ADR-017-consequence-gating.md"
    - precedent: "budget_tracking RAII reservation (ADR-008)"
    - precedent_wiring: ".pi/issues/precondition-followups/02-activate-composition-roots.md"

  acceptance_criteria:
    - "Reachability test fails if the seam stops calling reserve (the test is falsifiable)"
    - "A refused reservation blocks the step and records why"
    - "A permitted step commits; a failed step releases; a dropped guard releases"
    - "Unavailable (control plane down) refuses rather than allowing"
    - "Undeclared resources take exactly the previous code path (no behaviour change)"
    - "No regression in existing enforcement tests"

  validators:
    - ci
    - tests
    - architecture
    - integration

  implementation_notes: |
    - The RAII guard is the important structure: commit on success, release on drop. If release
      is only on an explicit error branch, a panic or early return leaks capacity until the
      lease expires — a real availability bug.
    - Do not put a "provider said allow" style short-circuit anywhere: the ledger's grant is
      evidence that capacity was reserved, not authority to skip other rules.
    - Refusal must be non-retryable; retrying a capacity refusal is a hot loop.

  file_changes:
    - "modify: engine/src/enforcement/application/enforcer_impl.rs"
    - "create: engine/src/capacity_ledger/application/guard.rs"
    - "modify: engine/src/orchestrator (composition root wiring)"
    - "create: engine/tests/integration/capacity-ledger/seam_reachability_test.rs"

  pipeline_steps:
    - implement
    - validate
    - create-mr
    - merge
---

# ISSUE-CAPACITY-LEDGER-SEAM — wire into the enforcement path

## Intent

This is where the feature becomes real. A port with adapters is still dead code until the
execution path calls it.

## Why this shape

The repository has already encountered the "implemented but not wired" failure class
(`precondition-followups/02-activate-composition-roots`, `03-activate-reachability-test`).
The lesson from those: **prove wiring with a reachability test**, not with unit coverage.

## In Scope

The seam, the RAII guard, refusal recording, opt-in scoping, and the reachability test.

## Out of Scope

Config surface, LLM-budget replacement, sequence policy.

## Acceptance Criteria

The reachability test must **fail if the call is removed** — otherwise it proves nothing.

## Notes

Release-on-drop is not optional. An explicit-error-only release leaks capacity until lease
expiry.
