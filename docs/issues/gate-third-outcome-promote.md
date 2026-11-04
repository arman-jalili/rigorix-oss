---
guardian_issue:
  id: "ISSUE-GATE-THIRD-OUTCOME-PROMOTE"
  title: "The spec gives a gate three outcomes; the new entry point has two, and a `promote` rule currently reads as 'may dispatch' — nothing in production ever sets `requires_approval`"
  epic: "portable-slice"
  component: "Engine"
  module: "gate-composition"
  status: planned
  priority: high
  tracking_issue: 1392
  dependencies:
    - "#1381 (the composition that flattens the third outcome)"
    - "#1382 (the CLI adoption that deleted the path which could have handled it)"

  intent: |
    The specification names three gate outcomes: *"The outcome of a gate: permit, promote (route to
    human approval), or refuse"* (`01-terminology.md:114`). The engine's new single entry point
    expresses two:

    ```rust
    // engine/src/gate/application/dto.rs:227
    pub enum GateVerdict { Dispatch, Refuse { gate, rule_id, reason } }
    ```

    And the composition folds the third into the first:

    ```rust
    // engine/src/gate/application/compose.rs:143-160
    let refused = matches!(finding.action, RequirementAction::Deny);
    findings.push(GateFinding {
        outcome: if refused { GateOutcome::Refused } else { GateOutcome::Passed },
        …
    });
    ```

    A finding whose action is `RequirementAction::Promote` — documented as *"Set
    `requires_approval = true` on the matched step"* (`sequence_policy/domain/requirement.rs:47`) —
    is recorded as **Passed**, and the verdict stays **Dispatch**. The promotion survives only inside
    the opaque `evidence` JSON of the finding, which a caller reading `verdict` never sees.

    It is worse than a label: **nothing outside doc comments and tests ever sets `requires_approval`.**

    ```
    $ grep -rn 'with_requires_approval|requires_approval = true' engine/src cli/src mcp/src \
        | grep -viE 'test|dag_engine/domain/graph.rs'
    engine/src/sequence_policy/mod.rs:15          (a doc comment)
    engine/src/sequence_policy/application/companion.rs:40   (a doc comment)
    engine/src/sequence_policy/application/service.rs:19     (a doc comment)
    …no assignment
    ```

    So an operator can write a rule that says *require human approval for this step*, and the
    composition reports "passed, dispatch". The approval mechanism itself exists — `requires_approval`
    on a plan node drives the pause/resume chain (ADR-011) — but no code connects a `promote`
    requirement to it.

    This is the concrete cost of two vocabularies for one axis: the spec's third value had no home in
    the type, so the mapping had nowhere to put it, and `#[deny]`-style exhaustiveness could not help
    because a two-valued enum with a `matches!(…)` check has no missing arm to fail on.

  in_scope:
    - "`GateVerdict` gains `Promote { gate, rule_id, reason }`; `GateOutcome` gains `Promoted`; the decision digest covers the third outcome, so two decisions differing only by promotion are not the same decision"
    - "The composition maps `RequirementAction::Promote` to `Promoted` and to a `Promote` verdict, never to `Passed`/`Dispatch`"
    - "The **dispatch path** applies the promotion: a step with a `Promote` verdict must not dispatch until the approval chain says so. The gate itself does not mutate the plan — it reports, the dispatcher acts. Where the CLI has no approval path at that point it must refuse with a reason naming the missing capability, not dispatch"
    - "The crate's `StepDecision` gains the third value (`Promoted`) and the wire contract version records it; the demo's honesty panel may then show a promotion it actually produced rather than describing one it cannot"

  out_of_scope:
    - "Re-implementing the approval pause/resume chain. It exists (ADR-011) and is out of the slice; the browser cannot pause for a human, so a `Promote` there is a **refusal with that reason**, stated as such"
    - "Changing `RequirementAction`'s variants or the rule TOML"
    - "The spec's wording. It is already right; the engine catches up to it (C.7 cites the corrected behaviour)"

  affected_layers:
    domain:
      - "`GateVerdict`, `GateOutcome` — one variant each"
    application:
      - "`gate/application/compose.rs` — the mapping"
      - "the dispatch path — applying the pause"
    infrastructure:
      - "None"
    interfaces:
      - "The crate's `StepDecision` projection and its wire contract version"

  citations:
    - file: "engine/src/gate/application/dto.rs"
      note: ":227 `GateVerdict` (two variants); `GateOutcome` beside it"
    - file: "engine/src/gate/application/compose.rs"
      note: ":143-160 the `RequirementAction::Deny` check that folds Promote into Passed"
    - file: "engine/src/sequence_policy/domain/requirement.rs"
      note: ":43 `RequirementAction { Deny, Promote }`; :47 the documented effect"
    - file: "docs/spec/execution-governance/01-terminology.md"
      note: ":107 and :114 — permit, promote, refuse"
    - file: "engine/src/dag_engine/domain/graph.rs"
      note: ":508-511 the `requires_approval` field and the pause contract it feeds"

  verification:
    - "Seed: a sequence-policy rule with `action = \"promote\"` matching the step → `GateVerdict::Promote`, and `GateOutcome::Promoted` — not `Dispatch`/`Passed`. The seed is the current behaviour as a failing test"
    - "The digest differs between a decision that promotes and one that dispatches: two evaluations of the same step under different rules do not produce the same `decision_digest`"
    - "CLI: a step under a promote rule does not dispatch before approval — seeded by running the CLI with a promote rule and asserting the run stops (or refuses naming the missing approval path)"
    - "Crate: a `Promote` verdict projects to `StepDecision::Promoted`, and a host reading only `verdict` can tell all three apart"
    - "`grep -rn 'requires_approval = true' engine/src cli/src | grep -v test` shows at least one non-doc assignment, or the dispatch path's refusal names why it cannot honour it"
---

# The third value had nowhere to live

Two vocabularies for one axis is not a style question. The spec's `promote` had no variant in
`GateVerdict`, so the mapping had no arm to fill, and `matches!(action, Deny)` cannot report a value
it does not know. A three-valued axis squeezed into two produces exactly one symptom: the missing
value is absorbed by the permissive one.

That absorption is the defect. Everything else in this issue follows from it.
