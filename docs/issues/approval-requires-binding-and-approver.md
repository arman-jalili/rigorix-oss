---
guardian_issue:
  id: "ISSUE-APPROVAL-REQUIRES-BINDING-AND-APPROVER"
  title: "approval: an approval requires an intent binding and an approver id in every mode; anything less is refused with a diagnosis"
  epic: "consequence gating"
  component: "ExecutionEngine"
  module: "approval"
  status: planned
  priority: high
  dependencies:
    - "ADR-011 — intent-bound approval (R1/R2/R3)"

  intent: |
    Every granted approval is intent-bound and attributed. There is one approval fact, one
    shape, and no mode in which an approval is granted without it.

    Today the branch decides the evidence: with an approval binding the engine publishes
    `ApprovalRecorded` per persisted record; without one it does
    `approved = candidate_names` and publishes nothing, and `input.approver_id` is never
    read. In that default mode an approval changes in-memory session state and leaves no
    trace in the signed envelope — verified on a `cost-demo` receipt for an
    approved-then-refused run, which contains no `approvals[]`, no `identity`, and no
    string containing `approv`.

    After this change an approval that cannot be bound and attributed does not happen.

  in_scope:
    - "The approval binding is required: not requested, or requested without a usable key, refuses the approval"
    - "An approval without an approver id is refused in every mode, not only when a binding happens to exist"
    - "Every refusal names the step and the reason, including the command that fixes the configuration"
    - "A step whose intent cannot be resolved is refused by name — never an internal error"
    - "`rigorix doctor` reports the active approval mode and the remediation"
    - "Upgrade and release notes state that approving now requires provisioning a key"
    - "Unit and integration tests for each refusal and for the granting path"

  out_of_scope:
    - "The binding algorithm, canonicalisation or TTL"
    - "Key distribution and rotation"
    - "The demo repositories — they are updated once this lands"
    - "An unbound approval event of any kind; the state it would describe is no longer reachable"

  affected_layers:
    domain:
      - "Modified: approval refusal carries a typed reason (missing binding, missing identity, unresolvable intent)"
    application:
      - "Modified: ExecutionService approve path — the non-binding branch refuses instead of granting"
      - "Modified: ApprovalBindingSetup construction — required, fallible, distinguished from 'not requested'"
    infrastructure:
      - "None"
    api:
      - "Modified: approve response denies with a reason per step"
      - "Modified: doctor reports the approval mode"

  canonical_references:
    - adr: ".pi/architecture/decisions/ADR-011-approval-binding.md"
    - module: ".pi/architecture/modules/audit.md#envelope"
    - last_sync: "2026-10-31"
    - evidence: "engine/src/execution_engine/application/service_impl.rs:1300 — the `else { approved = candidate_names }` branch grants without a record"
    - evidence: "engine/src/execution_engine/application/service_impl.rs:1359 — the `None =>` arm already denies when an approver id is missing, but only inside the binding branch"
    - evidence: "engine/src/audit/application/envelope_factory_impl.rs:606 — an approval ref requires node_id, step_name, intent_hash and approver_id"
    - evidence: "cost-demo receipt, approved-then-refused run: 4 events, no approval evidence of any kind"

  acceptance_criteria:
    - "With no binding configured, every requested step is denied and the refusal names RIGORIX_APPROVAL_BINDING, RIGORIX_HMAC_KEY and `rigorix key`"
    - "With no approver id, the refusal names the missing identity in every mode"
    - "An unresolvable intent refuses that step by name with a reason, not an internal error"
    - "With binding, key and approver id, the envelope carries one approval ref derived from the published event, plus identity"
    - "A seeded test proves each refusal fires and that the granting path still grants"
    - "doctor reports the active approval mode and the remediation command"
    - "The release note states the behaviour change and what an operator must do"

  validators:
    - ci
    - tests
    - security

  implementation_notes: |
    - Refusing is the decided behaviour; the question of representing an unbound approval is
      closed. Do not add an event type, a field or a mode for it.
    - The distinction that matters is 'not requested' vs 'requested but unusable' vs 'armed'.
      A silent fall from the third to the first is what this issue removes; a refusal that
      does not say why will be read as broken software and switched off, so the reason is part
      of the deliverable, not decoration.
    - The deny path currently returns names only. Give it a reason so the MCP and CLI surfaces
      can carry the diagnosis.
    - Prefer the type over the guard: make the granting path require the binding and the
      approver id, so the unbound branch cannot be reached, rather than checking for them.

  file_changes:
    - "modify: engine/src/execution_engine/application/service_impl.rs"
    - "modify: engine/src/execution_engine/application/factory.rs"
    - "modify: mcp/src/host/mod.rs"
    - "modify: cli/src/cli_boundary/dispatch.rs"
    - "modify: cli/src/cli_boundary/doctor.rs"
    - "create: engine/tests/unit/approval/approval_requires_binding_test.rs"
    - "modify: docs/upgrade-notes.md"

  pipeline_steps:
    - implement
    - validate
    - create-mr
    - merge
---

# ISSUE-APPROVAL-REQUIRES-BINDING-AND-APPROVER

## The defect

```rust
if let Some(binding) = &self.approval_binding {
    // … approve(), publish ApprovalRecorded per record, deny names that did not bind
} else {
    approved = candidate_names;
}
```

`input.approver_id` is read only inside the first branch. The mode therefore decides the
evidence: with a binding the record says who approved and which intent was bound; without
one it says nothing at all, and the approval is still granted.

Measured on an artifact, not inferred from the code. The `cost-demo` run in scene 2 is a
human approval of a +21.7% commit, refused at the gate afterwards. Its receipt:

| field | value |
|---|---|
| `events` | `precondition_checked`, `precondition_checked`, `node_failed` |
| `approvals` / approval refs | absent |
| `identity` | absent |
| any string containing `approv` | none |

The product's sentence about that scene is "a human approves +21.7%, and the gate still
refuses". The receipt supports the second half only.

## The required end state

An approval is granted only when it can be bound to a resolved intent and attributed to an
approver. Otherwise it is refused, and the refusal says which of the two is missing and what
the operator must run. There is no third state in which an approval is granted unbound,
because a step approved without a pinned intent is a step whose dispatch may differ from what
was approved.

## Where it changes

| layer | change |
|---|---|
| engine / approve | the non-binding branch refuses; a missing approver id refuses in every mode; the deny result carries a reason |
| engine / setup | the binding is required and fallible; "not requested" and "requested but unusable" are different outcomes |
| mcp, cli | the refusal reason reaches the caller; doctor reports the active mode and the remediation |
| docs | upgrade note: approving now requires `rigorix key` and the exported key |

## How it is proven

- One test per refusal: no binding, no approver id, unresolvable intent — each asserts the
  step is denied **and** the reason names what is missing.
- One test for the granting path: binding + key + approver id, and the envelope contains one
  approval ref and the identity.
- A receipt-level assertion, not only an in-memory one: the approval appears in the signed
  file a recipient gets.
