---
guardian_issue:
  id: "ISSUE-COMPANION-OBLIGATION-IS-PRESENCE-ONLY"
  title: "sequence_policy: the R3 companion obligation is satisfied by a step existing anywhere in the plan — not by ordering, dependency, or success"
  epic: "consequence gating"
  component: "CompanionStepObligation"
  module: "sequence-policy"
  status: planned
  priority: high
  dependencies:
    - "ADR-017 R3 — a consequential step may require a companion step"
    - "#946 / #973 — CompanionStepObligation implementation"

  intent: |
    Make the obligation mean what an operator reads it to mean. Today `evaluate_plan` marks the
    obligation satisfied when a matching companion appears **anywhere** in the step list. It does
    not check that the consequential step depends on the companion, that the companion runs
    first, or that the companion succeeded.

    In a DAG where both steps are independent, the obligation is satisfied and the two may run
    concurrently — or the consequential step may run first. The rule reads as "the payment
    cannot proceed unless the authority recheck happens", and enforces only "a recheck step is
    present in this file".

  in_scope:
    - "Define the two semantics explicitly: presence obligation vs execution prerequisite"
    - "For the prerequisite semantics, require a real dependency and a successful outcome"
    - "Refuse or report a companion that is not scheduled before the consequential step"
    - "Cover the concurrent case: two independent steps must not satisfy a prerequisite"
    - "Cover the failure case: a failed companion must not satisfy it"
    - "Document which semantics each configured obligation uses"

  out_of_scope:
    - "Adding new obligation kinds beyond ordering/dependency/success"
    - "Changing the wire names of existing findings"

  affected_layers:
    domain:
      - "Modified: CompanionStepObligation — the requirement kind"
    application:
      - "Modified: evaluate_plan — order/dependency/success-aware evaluation"
      - "Modified: dispatch — consult the outcome of the companion when required"
    infrastructure:
      - "None"
    api:
      - "Modified: finding payload — name the unmet requirement (absent vs not-before vs failed)"

  canonical_references:
    - adr: ".pi/architecture/decisions/ADR-017-consequence-gating.md"
    - evidence: "engine/src/sequence_policy/application/companion_impl.rs — the in-code comment states 'A companion present anywhere in the same plan satisfies the obligation'"
    - evidence: "engine/src/sequence_policy/application/companion_impl.rs — evaluate_plan takes &[PlannedStep] only; no execution state, no edges"
    - evidence: "engine/src/sequence_policy/application/companion_impl.rs — the inner loop tests candidate.tool/parameters and breaks on first match"

  acceptance_criteria:
    - "A plan whose companion is listed after the consequential step does not satisfy a prerequisite"
    - "A plan where the two steps are independent (both eligible) does not satisfy a prerequisite"
    - "A plan where the companion failed does not satisfy a prerequisite"
    - "Presence-only obligations remain expressible and documented as such"
    - "Findings distinguish absent / unordered / failed companion"

  validators:
    - ci
    - tests
    - architecture

  implementation_notes: |
    - The name is the bug: 'companion step' sounds like a prerequisite. Decide the semantics per
      obligation and put the decision in the type, not in a doc comment.
    - evaluate_plan is plan-time, so it cannot see success. A prerequisite semantics needs the
      check at dispatch time, where the companion's TaskResult is known. That is a different
      seam from the current one, which is the point of the issue.
    - Keep the presence check if it is genuinely wanted for cheap plan-time linting; just do not
      let it be the only meaning, and do not let it claim the stronger guarantee.
    - Precondition gating (ADR-017) is the mechanism that already re-reads authoritative state
      at dispatch. If the real requirement is "authority was rechecked", the precondition gate
      may be the right home; the companion rule may only need to enforce ordering.

  file_changes:
    - "modify: engine/src/sequence_policy/application/companion_impl.rs"
    - "modify: engine/src/sequence_policy/domain/*.rs"
    - "create: engine/tests/unit/sequence_policy/companion_order_test.rs"
    - "modify: engine/.pi/architecture/modules/sequence-policy.md"

  pipeline_steps:
    - implement
    - validate
    - create-mr
    - merge
---

# ISSUE-COMPANION-OBLIGATION-IS-PRESENCE-ONLY

## The defect

```rust
// companion_impl.rs
// A companion present anywhere in the same plan satisfies the
// obligation (plan-time, pre-side-effect).
let mut companion_present = false;
for candidate in steps {
    if obligation.require_companion_step.matches(&candidate.tool, &candidate.parameters)? {
        companion_present = true;
        break;
    }
}
```

The comment is honest about the implementation. The problem is what the feature is named and
how it reads: a *companion step requirement* on a *consequential* step implies the consequence
is gated on the companion.

It is not. `evaluate_plan` sees `&[PlannedStep]` — tools and parameters, no edges, no order, no
outcomes. So:

| plan | satisfied? | should it be? |
|---|---|---|
| companion listed before, has an edge | yes | yes |
| companion listed after | **yes** | no |
| companion independent; both eligible concurrently | **yes** | no |
| companion present but failed | **yes** | no |

## Why it matters

This is the same shape as the other findings in this area: a control that exists, is configured,
appears in the feature list, and does not enforce the property its name asserts. A plan author
who needs *"the payment cannot execute unless the authority recheck succeeded"* gets *"include a
recheck step somewhere"*, which is satisfiable by a plan that never runs it first.

## Suggested fix

Split the concept:

- **Presence obligation** — plan-time lint. Keep it, name it `requires_step_present`.
- **Execution prerequisite** — checked at dispatch against the companion's recorded outcome and
  the DAG edges. This is the guarantee the word "companion" implies.

The prerequisite cannot live in `evaluate_plan` because success is not known there. It belongs
at the dispatch seam, next to the precondition gate — which is also the mechanism that already
re-reads authoritative state at the moment of consequence.
