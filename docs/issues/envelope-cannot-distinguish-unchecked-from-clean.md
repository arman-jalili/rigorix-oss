---
guardian_issue:
  id: "ISSUE-ENVELOPE-UNCHECKED-VS-CLEAN"
  title: "the signed envelope records scope violations but not whether the check ran — 'no violations' is indistinguishable from 'never checked'"
  epic: "consequence gating"
  epic_id: "EPIC-CONSEQUENCE-GATING"
  tracking_issue: 1189
  component: "Audit"
  module: "approval"
  status: planned
  priority: high
  created_at: "2026-10-23T19:00:00Z"
  updated_at: "2026-10-23T19:00:00Z"

  intent: |
    Record the scope check's *outcome class* on the envelope, so a verifier can tell a clean check from
    an absent one.

    The envelope carries `scope_violations: Vec<ScopeViolationRef>` with
    `skip_serializing_if = "Vec::is_empty"` (`audit/domain/envelope.rs:242-244`). The dispatch-time check
    returns early in three situations, all silent (`dispatch.rs:462-500`):

    - no declared scope (`if declared.is_empty() { return; }`)
    - the oracle is unavailable (`let Ok(post) = oracle.snapshot(..) else { return; }`)
    - there are no effects in the diff

    Every one of those produces the same envelope shape as a clean check: the field is absent. So the
    signed record cannot answer "was this run's effect scope verified?" — only "were any violations
    found", which is the weaker question and the wrong one for a verifier.

    The runbook already promises the marker (`runbook-approval.md:82`: "skip with an explicit
    `scope_verification: skipped` marker in the envelope — **never silent**"), but
    `scope_verification` appears nowhere in the code.

    This is the same principle as the authority-fact projection: absence of evidence must be
    distinguishable from evidence of absence, or the record overstates what happened.

  dependencies:
    - name: "EffectScopeOracle / effect_scope_check"
      type: internal
      note: "dispatch.rs:462-500 — three early returns; approval/infrastructure/effect_scope.rs."
    - name: "envelope scope_violations"
      type: internal
      note: "audit/domain/envelope.rs:242-244 — violations only, omitted when empty."
    - name: "runbook marker"
      type: internal
      note: "runbook-approval.md:82 — documents `scope_verification: skipped`; not implemented."

  in_scope:
    - "Add `scope_verification` to the envelope: `checked` | `not_declared` | `unavailable`"
    - "Record it per approval-gated step (or per run with a step list), populated at dispatch"
    - "Emit it even when the result is clean — that is the point"
    - "Implement the runbook's `skipped` as `unavailable` (or keep the word; pick one and align the doc)"
    - "Test: no declared scope ⇒ envelope says `not_declared`, not nothing"

  out_of_scope:
    - "Producing scopes for command steps (separate issue: the approve surface)"
    - "Making violations blocking"
    - "Envelope versioning beyond an additive, serde-defaulted field"

  affected_layers:
    domain:
      - "Modified: ExecutionEnvelope — additive field"
    application:
      - "Modified: effect_scope_check records its outcome; envelope factory carries it"
    infrastructure:
      - "None"
    api:
      - "The signed record gains a field; additive and defaulted"

  canonical_references:
    - module: ".pi/architecture/modules/approval.md"
    - adr:
        - ".pi/architecture/decisions/ADR-011-approval-binding.md"
        - ".pi/architecture/decisions/ADR-016-audit-integrity-anchor.md"
    - last_sync: "2026-10-23"
    - evidence: "audit/domain/envelope.rs:242-244 — scope_violations, skip_serializing_if empty"
    - evidence: "execution_engine/application/service_impl/dispatch.rs:462-500 — three silent early returns"
    - evidence: "runbook-approval.md:82 — documents a `scope_verification: skipped` marker"
    - evidence: "no `scope_verification` reference anywhere in engine/src"

  acceptance_criteria:
    - "A gated step with no declared scope produces `scope_verification: not_declared` in the envelope"
    - "A gated step whose oracle is unavailable produces `unavailable`, not silence"
    - "A clean check produces `checked` (with an empty violations list)"
    - "The field is additive and serde-defaulted; old envelopes still verify"
    - "A verifier can distinguish all three states from the signed record alone"
    - "validate-ci.sh, validate-tests.sh, validate-canonical.sh pass"

  validators:
    - architecture-validator
    - ci
    - tests
    - canonical

  implementation_notes: |
    Three states, not a boolean: `checked`, `not_declared`, `unavailable` (oracle down). A caller that
    conflates the last two cannot tell "we chose not to look" from "we could not look", and those have
    different operational remedies.

    Emit `checked` even when clean. A field that appears only on failure teaches a verifier that absence
    means success — the exact inference this issue exists to prevent.

    Keep the envelope additive: `#[serde(default)]` plus a `skip_serializing_if` that keeps old
    envelopes byte-identical when unset, and a version bump only if the verifier needs to distinguish
    "absent because old" from "absent because unchecked" — in which case a sentinel `unknown` is
    clearer than a missing field.

  file_changes:
    - "modify: engine/src/audit/domain/envelope.rs"
    - "modify: engine/src/audit/application/envelope_factory_impl.rs"
    - "modify: engine/src/execution_engine/application/service_impl/dispatch.rs"
    - "create: engine/tests/scope_verification_recorded.rs"
    - "modify: engine/docs/runbook-approval.md"

  pipeline_steps:
    - implement
    - validate
    - create-mr
    - merge
---

# ISSUE-1189: "no violations" is not "checked"

## What you are looking at

| situation | envelope today | what a verifier can conclude |
|---|---|---|
| scope declared, clean | `scope_violations` absent | ? |
| no scope declared | `scope_violations` absent | ? |
| oracle unavailable | `scope_violations` absent | ? |
| violations found | list present | violations found |

Three different facts, one representation. The runbook already promises the marker; the code has none.

## In Scope

- `checked` / `not_declared` / `unavailable`, emitted always, additive to the envelope
- Runbook alignment (`skipped` ⇒ pick one word)
- Test for the no-scope case

## Out of Scope

- Producing scopes for commands (next issue); blocking violations; envelope versioning beyond additive

## Affected Layers

### Domain
- `ExecutionEnvelope` gains the field

### Application
- The check records its outcome; the factory carries it

## Canonical References

- **Module:** `.pi/architecture/modules/approval.md`
- **ADR-011, ADR-016**
- **Evidence:** `envelope.rs:242-244`; `dispatch.rs:462-500`; `runbook-approval.md:82`

## Acceptance Criteria

| # | Criterion | Validator |
|---|-----------|-----------|
| 1 | No scope ⇒ `not_declared` | `validate-tests.sh` |
| 2 | Oracle down ⇒ `unavailable` | `validate-tests.sh` |
| 3 | Clean ⇒ `checked` | `validate-tests.sh` |
| 4 | Old envelopes still verify | `validate-integration.sh` |
| 5 | Three states distinguishable from the record | `validate-canonical.sh` |

## Implementation

1. Read `envelope.rs:230-260`, `dispatch.rs:462-500`, `envelope_factory_impl.rs:150-175`.
2. Add the field; populate from the check; keep it additive.
3. Test the three paths; align the runbook wording.

## Pitfalls

- **Do not emit only on failure** — that recreates the inference "absent means clean".
- **Do not use a boolean.** "Chose not to look" and "could not look" differ.
- **Keep the record verifiable**: additive + defaulted, and say which value old envelopes mean.

## Why this is high

It is a provenance hole in the signed artifact: a downstream verifier reading a clean-looking envelope
cannot tell whether the scope control ran at all, which is precisely the claim the effect-detection
dimension is scored on.
