---
guardian_issue:
  id: "ISSUE-APPROVAL-STORE-IS-UNPROTECTED-STATE"
  title: "approval: the store that decides whether an approval is still valid and unused is unsigned plaintext inside the agent-writable workspace"
  epic: "consequence gating"
  component: "ApprovalBinding"
  module: "approval"
  status: planned
  priority: high
  tracking_issue: 1237
  dependencies:
    - "ADR-011 — intent-bound approval (single-use, TTL)"

  intent: |
    The lifecycle of an approval — still pending, still unexpired, not yet consumed — is
    authority over whether a dispatch may happen. It must not be editable by the process it
    controls.

    Approval records are persisted to `<repo_root>/.rigorix/approvals.json`, inside the
    directory tree the agent writes. The record carries `status`, `expires_at` and `nonce` as
    plain fields, and the only keyed value is `intent_hash`, which binds the intent and
    nothing else. So single-use and TTL are enforced by a file whose contents nothing
    verifies: a record that was consumed can be set back to `pending`, an expired one can be
    given a later `expires_at`, and the next dispatch verifies a record that the human's
    approval no longer describes.

    After this change a record whose lifecycle fields do not verify is refused, and the store
    can be placed outside the workspace that the agent may write.

  in_scope:
    - "A MAC over the record's lifecycle fields, derived from the same run key, verified on load"
    - "A verification failure refuses the dispatch with a diagnosis naming the record and the step — never a silent re-approval"
    - "The store's location is configurable, so it can resolve outside the agent-writable workspace"
    - "`rigorix doctor` reports the store path and whether the records verify"
    - "Tests: a consumed record edited back to pending is refused; an extended `expires_at` is refused; an untouched record still verifies"

  out_of_scope:
    - "The intent hash, which already binds the payload and is keyed correctly"
    - "Key distribution and rotation"
    - "Multi-process locking, which is a separate concurrency question"

  affected_layers:
    domain:
      - "Modified: ApprovalRecord — lifecycle fields covered by a verifiable MAC"
    application:
      - "Modified: verify_intent — a record that fails verification is Invalid, not Matched"
    infrastructure:
      - "Modified: approval repository — MAC written on persist, checked on load; configurable store path"
    api:
      - "Modified: doctor reports the store path and verification status"

  canonical_references:
    - adr: ".pi/architecture/decisions/ADR-011-approval-binding.md"
    - last_sync: "2026-10-31"
    - evidence: "engine/src/execution_engine/application/factory.rs:287 — records persist to `<repo_root>/.rigorix/approvals.json`"
    - evidence: "engine/src/approval/domain/record.rs:127 — status, expires_at, nonce are plain fields; no MAC is declared"
    - evidence: "engine/src/approval/application/service_impl.rs:76 — the keyed digest covers the canonical intent, not the record's lifecycle"
    - evidence: "engine/src/approval/application/service_impl.rs — verify_intent compares digests and returns Matched; it does not authenticate the record"

  acceptance_criteria:
    - "A record edited after it was written is refused at dispatch, with a reason"
    - "A consumed record restored to pending is refused"
    - "An expired record with a rewritten expires_at is refused"
    - "An unmodified record still verifies and dispatches"
    - "The store path is configurable, and doctor prints it"
    - "Each refusal is seeded, and each seed fails if the check is removed"

  validators:
    - ci
    - tests
    - security

  implementation_notes: |
    - The rule this violates is already in the repo: an artifact a decision depends on must
      resolve outside the workspace the agent can write. The precondition check artifact was
      moved for this reason; the approval store is the same shape of authority and was not.
    - A MAC alone does not fix the deletion case — deleting the record yields NotFound and a
      re-approval, which fails closed. It fixes the edit case, which is the one that produces
      a dispatch: consumed back to pending, or a later expiry.
    - Verify on load, not on consume: a record that fails verification must never reach the
      comparison that decides Matched.
    - Prefer the type: a verified record and an unverified one should not be the same type.

  file_changes:
    - "modify: engine/src/approval/domain/record.rs"
    - "modify: engine/src/approval/application/service_impl.rs"
    - "modify: engine/src/approval/infrastructure/repository/approval_repository_impl.rs"
    - "modify: engine/src/execution_engine/application/factory.rs"
    - "modify: cli/src/cli_boundary/doctor.rs"
    - "create: engine/tests/unit/approval/approval_store_integrity_test.rs"

  pipeline_steps:
    - implement
    - validate
    - create-mr
    - merge
---

# ISSUE-APPROVAL-STORE-IS-UNPROTECTED-STATE

## The defect

```rust
// engine/src/approval/domain/record.rs:127
pub struct ApprovalRecord {
    pub step_name: String,
    pub node_id: Uuid,
    pub intent_hash: IntentHash,       // keyed
    pub intent_payload: serde_json::Value,
    pub approver_id: String,
    pub authority: Option<String>,
    pub decided_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,      // plain
    pub nonce: Uuid,                    // plain
    pub status: ApprovalStatus,         // plain
    pub decision_context: DecisionContext,
}
```

and the store:

```
<repo_root>/.rigorix/approvals.json      // factory.rs:287
```

The keyed value is `intent_hash`, computed over the canonical intent. Nothing covers
`status`, `expires_at` or `nonce` — the three fields that answer "may this dispatch happen
now, with this approval, once". They are written as JSON and read back as JSON.

## Why that is the defect

The approval's lifecycle is the authority. Whoever can write the file can say "this consumed
approval is pending again" or "this expired approval lasts another day", and the next
dispatch will verify a record that the human's approval no longer describes. That is the
failure ADR-011's single-use rule exists to prevent, enforced by a plaintext file inside the
tree the agent writes.

The repo already ruled on this shape: a precondition check artifact must resolve outside the
workspace the agent can write, and it was moved for exactly this reason. The approval store
is the same kind of authority and was left inside.

## The required end state

A record that does not verify is refused. Verification covers the lifecycle fields with a MAC
derived from the run key, happens on load, and a failure is a refusal with a diagnosis — not a
silent re-approval, and not a comparison that proceeds to `Matched`. Separately, the store's
path is configurable so a deployment can place it outside the writable workspace.

## How it is proven

| seed | expected |
|---|---|
| a consumed record edited back to `pending` | dispatch refused; reason names the record and the step |
| a record with `expires_at` moved into the future | refused |
| a record read back unmodified | verified, and the approval dispatches |
| the MAC check removed | the first two seeds pass silently — the test fails |
| the store moved outside the workspace | doctor reports the new path |
