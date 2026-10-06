---
guardian_issue:
  id: "ISSUE-PF-REL-2"
  title: "REL (optional): relocate R3 companion-step obligation into sequence_policy"
  epic: "precondition-followups"
  epic_id: "EPIC-PRECONDITION-FOLLOWUPS"
  component: "RelocateR3"
  module: "sequence-policy"
  status: planned
  priority: low
  dependencies:
    - "ISSUE-PF-REN-2"

  in_scope:
    - "Move CompanionStepObligation into sequence_policy as an extension of ADR-015 [[requirements]]"
    - "Reuse StepPredicate + the requirement/finding machinery (requirement_findings[] evidence)"
    - "Keep the config semantics (require_companion_step) unchanged"

  out_of_scope:
    - "Behavior change"
    - "R1/R2"

  affected_layers:
    domain:
      - "Move: companion obligation -> sequence_policy/domain/requirement.rs"
    application:
      - "Modify: sequence_policy/application/service_impl.rs (evaluate_requirements)"

  canonical_references:
    - adr: "engine/.pi/architecture/decisions/ADR-015-operator-step-requirements.md"
    - module: "engine/.pi/architecture/modules/consequence-gating.md#r3"

  acceptance_criteria:
    - "Companion-step evaluation is owned by sequence_policy and reuses requirement_findings[]"
    - "precondition module no longer owns the companion-step evaluator"
    - "Existing companion tests pass under the new home"

  validators:
    - ci
    - tests
    - architecture

  implementation_notes: |
    R3 is a plan-composition requirement (ADR-015), not a dispatch-time check.
    It already reuses StepPredicate and the requirement pattern. Relocation is
    optional; if deferred, close with rationale.

  file_changes:
    - "move: engine/src/precondition/application/{companion,companion_impl}.rs -> engine/src/sequence_policy/application/"
    - "modify: engine/src/sequence_policy/application/service_impl.rs"
---

# ISSUE-PF-REL-2: Relocate R3

Optional DDD cleanup. The companion-step obligation is ADR-015's
`[[requirements]]` mechanism; it belongs in `sequence_policy`, reusing the
existing requirement + `requirement_findings[]` machinery.
