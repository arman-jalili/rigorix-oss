---
guardian_issue:
  id: "ISSUE-DECLARED-SCOPE-HAS-NO-PRODUCER"
  title: "the effect-scope oracle is wired in every root and nothing produces a declared_scope — so the scope control cannot fire, and the docs say it is by design"
  epic: "consequence gating"
  epic_id: "EPIC-CONSEQUENCE-GATING"
  tracking_issue: 1175
  component: "Approval"
  module: "approval"
  status: planned
  priority: high
  created_at: "2026-10-23T15:00:00Z"
  updated_at: "2026-10-23T15:00:00Z"

  intent: |
    Decide whether effect-scope detection is a shipped control or a designed one, and make the
    artifacts say the same thing.

    The oracle exists and is wired: `EffectScopeOracle` reads the approved intent's `declared_scope`,
    diffs it against the run's actual effects, and reports violations (`approval/infrastructure/effect_scope.rs`).
    `approval_repo_path: Some(repo_root) ` is set in all three roots (#1099), and the wiring-claims
    manifest records that as `W6 CONFIRMED`.

    Nothing produces a `declared_scope`. `ExecutionIntent::from_payload` reads the field out of a
    payload (`approval/domain/intent.rs:63-81`), and `dispatch.rs:482` reads it from the node's intent
    JSON — both are consumers. No producer exists: not the planner, not template generation, not the
    CLI/MCP input path, and no non-Rust source (template, toml, json) contains the key.

    The runbook is honest about the consequence (`runbook-approval.md:82`):

    > "The oracle is wired by every entry point (#1099) but inspects nothing unless the approved intent
    > declared a `declared_scope`; with no declared scope there is no effect-scope evidence, by design."

    So the code is not lying. What is misleading is the surrounding claim set: README's security table
    lists scope as a control ("a credential is used outside the step it was approved for"), and the
    release notes advertise effect detection. A control that cannot fire is not a control; it is a
    design. Either give it a producer or stop counting it.

  dependencies:
    - name: "ExecutionIntent::from_payload"
      type: internal
      note: "approval/domain/intent.rs:63-81 — reads declared_scope from a payload; consumer only."
    - name: "EffectScopeOracle"
      type: internal
      note: "approval/infrastructure/effect_scope.rs — gated on a non-empty declared_scope."
    - name: "three composition roots"
      type: internal
      note: "approval_repo_path wired (#1099); the oracle will run, it just has nothing to compare."

  in_scope:
    - "Decide: producer or retirement (see the decision below)"
    - "If producer: emit declared_scope where the plan is created, from the approved intent's stated files/globs"
    - "If retirement: remove the claim from README and the runbook, keep the oracle for approved intents that do carry scope"
    - "A test that the chosen path is observable: with a declared scope, an out-of-scope effect is reported"

  out_of_scope:
    - "Changing how the oracle diffs effects (it is tested)"
    - "The approval hash, which already binds `(tool, intent, declared_scope)`"
    - "Enterprise-side projection of scope facts"

  affected_layers:
    domain:
      - "Existing: ExecutionIntent.declared_scope — no change"
    application:
      - "New (if producer): the plan-creation path populates the field"
    infrastructure:
      - "None"
    api:
      - "Modified (if retirement): README and runbook wording"

  canonical_references:
    - module: ".pi/architecture/modules/approval.md"
    - adr:
        - ".pi/architecture/decisions/ADR-011-approval-binding.md"
        - ".pi/architecture/decisions/ADR-017-consequence-gating.md"
    - last_sync: "2026-10-23"
    - evidence: "approval/domain/intent.rs:63-81 — declared_scope parsed from a payload"
    - evidence: "execution_engine/application/service_impl/dispatch.rs:482 — reads it from the node intent JSON"
    - evidence: "no producer anywhere: no planner/template/CLI writer, no non-Rust source contains the key"
    - evidence: "runbook-approval.md:82 — 'inspects nothing unless the approved intent declared a declared_scope … by design'"
    - evidence: "README.md:290 — scope listed as a control"

  acceptance_criteria:
    - "The decision is recorded, and README + runbook + module doc agree with it"
    - "If a producer exists: a run with a declared scope reports an out-of-scope effect, end to end"
    - "If retired: no document claims scope enforcement as a control"
    - "The wiring-claims row (W6) describes what the oracle actually does in each case"
    - "validate-ci.sh, validate-tests.sh, validate-canonical.sh pass"

  validators:
    - architecture-validator
    - ci
    - tests
    - canonical

  implementation_notes: |
    **Recommendation: give it a producer.** The oracle, the intent hash, and the approval binding are
    all built; the only missing piece is that whoever writes the plan does not say which files the step
    is allowed to touch. The natural source is the node's own intent — a step that says "update
    README.md" declares `["README.md"]`, and a step that says nothing declares nothing (keeping
    today's behaviour for un-scoped steps).

    Do not fabricate a scope from the plan's dependency list or from the tool's permission. A guessed
    scope produces false violations, which is worse than no evidence: operators learn to ignore them.

    If you choose retirement instead, be strict about the wording: the oracle remains useful for
    approved intents that carry a scope (enterprise operators can set it), so the honest statement is
    "we do not derive a scope automatically; the control applies when one is declared" — not "scope
    enforcement is not implemented".

  file_changes:
    - "modify: engine/src/planning/ or template_generation (producer)"
    - "modify: README.md, engine/docs/runbook-approval.md"
    - "create: engine/tests/effect_scope_producer.rs (if producer)"

  pipeline_steps:
    - implement
    - validate
    - create-mr
    - merge
---

# ISSUE-1175: the scope oracle has no evidence source

## What you are looking at

```
approval declared_scope  ──┐
                           ├──► EffectScopeOracle ──► violations?  (wired in 3 roots)
actual effects (git diff) ─┘
```

The right-hand side is complete and tested. The left-hand top has no writer: `from_payload` and
`dispatch.rs:482` both *read* `declared_scope`, and nothing writes it. So the oracle runs, compares
against an empty list, and cannot report anything.

## Decision (recommended)

Produce a scope from the node's stated intent; do not invent one from the plan graph or permissions.
A guessed scope generates false violations and trains operators to ignore the control.

## In Scope

- The decision; the producer (or the retirement wording)
- One end-to-end test that an out-of-scope effect is reported

## Out of Scope

- Oracle diffing; the approval hash; enterprise projection

## Affected Layers

### Application
- Plan creation populates `declared_scope`

### API
- README / runbook wording if retired

## Canonical References

- **Module:** `.pi/architecture/modules/approval.md`
- **ADR-011 / ADR-017**
- **Evidence:** `intent.rs:63-81`, `dispatch.rs:482`, `runbook-approval.md:82`, `README.md:290`

## Acceptance Criteria

| # | Criterion | Validator |
|---|-----------|-----------|
| 1 | Docs agree with the decision | `validate-canonical.sh` |
| 2 | Producer path observable end to end | `validate-tests.sh` |
| 3 | W6 row describes actual behaviour | review |
| 4 | No claim of a control that cannot fire | `validate-canonical.sh` |

## Implementation

1. Read `effect_scope.rs`, `intent.rs`, `dispatch.rs:470-500`, `runbook-approval.md:75-90`.
2. Implement the chosen path.
3. Align README/runbook/module doc.
4. Add the end-to-end test.

## Pitfalls

- **Do not guess a scope.** False violations are worse than absent evidence.
- **Do not delete the oracle** if retiring; scoped intents still benefit.
- **Keep the W6 row true** — CONFIRMED describes wiring, not that the control engages.

## Why this is high

The README sells scope as a security control and the release notes advertise effect detection. Today
neither can fire in a normal run, and only one doc paragraph admits it.
