---
guardian_issue:
  id: "ISSUE-MAC-FIELD-LIST-IS-HAND-MAINTAINED"
  title: "approval: the record MAC covers a hand-written field list, so a field added to the record and forgotten there is silently unprotected"
  epic: "consequence gating"
  component: "ApprovalBinding"
  module: "approval"
  status: planned
  priority: high
  tracking_issue: 1240
  dependencies:
    - "#1237 — the authenticated approval store"

  intent: |
    The MAC is documented as covering "HMAC-SHA256 over every other field", and the
    implementation does not derive that set — it restates it:

    ```rust
    #[derive(Serialize)]
    struct MacMaterial<'a> {
        version: u8,
        step_name: &'a str,
        node_id: &'a Uuid,
        intent_hash: &'a IntentHash,
        intent_payload: &'a serde_json::Value,
        approver_id: &'a str,
        authority: &'a Option<String>,
        decided_at: String,
        expires_at: String,
        nonce: &'a Uuid,
        token_claims_ref: &'a Option<String>,
        status: ApprovalStatus,
        decision_context: &'a DecisionContext,
        approver_scope: &'a [String],
    }
    ```

    Today the two lists agree. Nothing makes them agree: adding a field to
    `ApprovalRecord` — the next `superseded_by`, or a `tenant`, or a `consumed_by` — leaves
    it outside the MAC, and every existing test still passes, because the seeds name
    `status`, `expires_at` and "untouched", not "every field".

    After this change the material is derived from the record itself, or a test compares the
    two field sets, so a new field is either covered or the build fails.

  in_scope:
    - "The MAC material is derived from the record's own serialisation minus `mac`, or a parity assertion holds the two sets equal"
    - "A seed that adds a field to the test's record shape fails while that field is not covered"
    - "The `version` byte stays, so a change in the material is a version change"

  out_of_scope:
    - "The MAC algorithm and the key"
    - "Anti-rollback, which is a different property (#TBD)"
    - "Legacy records without a MAC"

  affected_layers:
    domain:
      - "Modified: the material derives from the record, not from a parallel struct"
    application:
      - "None"
    infrastructure:
      - "None"
    api:
      - "None"

  canonical_references:
    - adr: ".pi/architecture/decisions/ADR-011-approval-binding.md"
    - last_sync: "2026-10-31"
    - evidence: "engine/src/approval/domain/record.rs — `MacMaterial` lists 14 fields by hand"
    - evidence: "engine/tests/approval_store_integrity_integration.rs — the seeds mutate status, expires_at, and nothing else"
    - evidence: "the doc comment claims 'every field except mac', which is a claim about a derivation that is not derived"

  acceptance_criteria:
    - "Adding a field to ApprovalRecord without covering it fails a test"
    - "Removing the derivation (back to a hand list) fails the same test"
    - "The existing three seeds still pass"
    - "A record written by the previous material still verifies, or its version bump forces re-approval and says so"

  validators:
    - ci
    - tests
    - security

  implementation_notes: |
    - The rule the repo already applies elsewhere: a catalogue that nobody is required to
      extend is complete exactly once. The variant table is compiler-enforced; this one is
      not.
    - Deriving from `serde_json` of the record minus `mac` is the smaller change and makes the
      doc comment true. It also removes the risk of a field being added to the material and
      not the struct.
    - Keep the version byte: changing the material invalidates every record, which must be a
      deliberate, documented step (pre-MAC records already require re-approval).

  file_changes:
    - "modify: engine/src/approval/domain/record.rs"
    - "modify: engine/tests/approval_store_integrity_integration.rs"

  pipeline_steps:
    - implement
    - validate
    - create-mr
    - merge
---

# ISSUE-MAC-FIELD-LIST-IS-HAND-MAINTAINED

## The defect

```rust
/// HMAC-SHA256 over every other field, keyed with the run key (#1237).
pub mac: Option<String>,
```

"Every other field" is the property the store relies on. The implementation restates the
list in a second struct — fourteen field references, maintained by hand. The day a field is
added to `ApprovalRecord` and not to `MacMaterial`, the MAC still verifies and the field is
editable: the exact tampering #1237 closed, reopened silently by an ordinary refactor.

Nothing catches it. The three seeds mutate `status`, `expires_at`, and leave the record
untouched; a fourteenth-and-a-half field is not in any of them.

## Why this is the serious version of the finding

A MAC that covers *most* fields is worse than none, because it is trusted as if it covered
all of them. The property is stated in a comment and believed by everyone downstream —
`verify_integrity`, the dispatch halt, doctor's "N record(s) verify".

## The required end state

The material derives from the record: serialise the record, drop `mac`, hash the rest. The
`version` byte stays so a material change is a version change. A seed that adds a field to
the record shape fails while that field is outside the material, and keeps failing if
somebody reintroduces a hand list.

## How it is proven

| seed | expected |
|---|---|
| add a field to the record, not to the material | fails: the field is not covered |
| restore a hand-written material list | the same seed fails |
| the three existing mutations | still refused |
| an untouched record | still verifies |
