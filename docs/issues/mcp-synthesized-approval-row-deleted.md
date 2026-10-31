---
guardian_issue:
  id: "ISSUE-MCP-SYNTHESIZED-APPROVAL-ROW-DELETED"
  title: "mcp: delete the synthesized approval_recorded row; the session claim belongs in identity, not in a second approval event"
  epic: "consequence gating"
  component: "McpHost"
  module: "audit-tools"
  status: planned
  priority: medium
  dependencies:
    - "ISSUE-APPROVAL-REQUIRES-BINDING-AND-APPROVER — with the binding required, the engine event always exists"

  intent: |
    One approval is one event. The MCP resume path currently synthesizes an
    `approval_recorded` row in addition to whatever the engine published, so an approved
    run with an attested session carries two events for one approval, keyed differently —
    the synthesized row has `step_name` and no `node_id`, the engine's has both, and
    `event_ref_identity` joins on the first present key, so the two are never equal and
    both survive.

    The row is also inert where it stands alone: `approval_refs_from_events` requires
    `intent_hash`, so a synthesized row can never become a signed approval ref. It is an
    event that looks like approval evidence and is not.

    After the binding is required, the engine publishes the event on every granted
    approval. The synthesized row has no remaining purpose and is deleted; the session's
    attestation stays in `identity`, which is where an attestation belongs.

  in_scope:
    - "Remove the synthesized approval_recorded events from the resume path"
    - "Keep the session claim in the envelope's identity field"
    - "A test asserts an approved resumed run carries exactly one approval_recorded"

  out_of_scope:
    - "The node floor, which is a different mechanism and still needed for a missed bus window"
    - "De-duplication by identity — it is removed along with the duplication it was for"
    - "Any new event type"

  affected_layers:
    domain:
      - "None"
    application:
      - "Modified: resume path's synthesized event projection — node floor only"
    infrastructure:
      - "None"
    api:
      - "None"

  canonical_references:
    - module: ".pi/architecture/modules/audit.md#envelope"
    - last_sync: "2026-10-31"
    - evidence: "mcp/src/host/mod.rs:656 — for step in approval.approved_steps() pushes a synthesized approval_recorded"
    - evidence: "mcp/src/host/mod.rs:660 — payload has step_name, approver_id, authority, decided_at; no node_id, no intent_hash"
    - evidence: "engine/src/audit/application/envelope_factory_impl.rs:606 — approval refs require node_id, step_name, intent_hash and approver_id"
    - evidence: "mcp/src/host/mod.rs:1120 — event_ref_identity prefers node_id, then step_name, so the pair never joins"

  acceptance_criteria:
    - "The resume path no longer synthesizes approval_recorded events"
    - "An approved resumed run's envelope contains exactly one approval event, the engine's, with its node_id, intent_hash and approver_id"
    - "The session claim still appears in identity when one is present"
    - "A test asserts the count is exactly one, from the written envelope"

  validators:
    - ci
    - tests

  implementation_notes: |
    - Delete, do not de-duplicate. The row exists to stand in for an engine event that the
      previous mode never published; once it is always published, the stand-in is a second
      claim about the same fact.
    - The node floor is not affected: it fills a window the bus can miss, which the approval
      event does not have.
    - The assertion belongs on the file, not on a read surface: read the written envelope and
      count the events.

  file_changes:
    - "modify: mcp/src/host/mod.rs"
    - "modify: mcp/tests/evidence_preservation_test.rs"

  pipeline_steps:
    - implement
    - validate
    - create-mr
    - merge
---

# ISSUE-MCP-SYNTHESIZED-APPROVAL-ROW-DELETED

## The defect

```rust
if let Some(claim) = session_claim.as_ref() {
    for step in approval.approved_steps() {
        synthesized.push(ExecutionEventRef {
            event_type: "approval_recorded".to_string(),
            payload: Some(json!({ "step_name": step, "approver_id": claim.subject, … })),
            …
        });
    }
}
```

Two facts get mixed here. The engine's `approval_recorded` says *this intent was approved by
this approver*; the synthesized row says *this session, holding this claim, approved these
steps on resume*. The second is an attestation about the resumer, and the envelope already
has a field for it — `identity`, which the same code path passes to `build_and_send`.

Because the identity function prefers `node_id` and the synthesized row has none, the pair
does not join: an approved run with an attested session records two approvals for one
approval. And because approval refs require `intent_hash`, the synthesized row can never
become one — so it is not weaker evidence, it is not evidence: a row that reads like an
approval and contributes nothing.

## The required end state

One approval, one event, published by the engine that granted it. The resumer's attestation
lives in `identity`. No synthesized approval row, no de-duplication machinery for it, no new
event type.

## How it is proven

- An approved resumed run: count `approval_recorded` in the written envelope — exactly one —
  and assert it carries `node_id`, `intent_hash` and `approver_id`.
- The same envelope still carries `identity` when a session claim exists.
- Seeding the synthesized row back in makes the count assertion fail.
