---
guardian_issue:
  id: "ISSUE-APPROVAL-REF-CANNOT-SAY-DERIVED-OR-ASSERTED"
  title: "an approval ref carries an intent hash without saying whether the engine derived it or the caller asserted it, and the schema has no field to say so"
  epic: "consequence gating"
  component: "AuditEnvelope"
  module: "audit"
  status: planned
  priority: high
  tracking_issue: 1243
  dependencies:
    - "ADR-011 — intent-bound approval (RS-011, RS-012)"
    - "rigorix-enterprise #439 — the enterprise binding, which asserts the hash"

  intent: |
    `ApprovalRecordRef.intent_hash` is described as "intent hash the approval is bound to", and
    RS-011 requires an approval to bind to the **canonical intent** — tool, resolved payload,
    declared scope — with RS-012 requiring the implementation to re-derive it immediately
    before dispatch and halt on mismatch.

    There are two ways a value can arrive in that field:

    - the re-deriving engine computed it from the sealed graph (the OSS path), or
    - a caller supplied it and an attesting service stamped a MAC over "this approver said
      this hash" (the enterprise path, which has no engine and never sees a resolved intent).

    Both are schema-valid, both verify, and the artifact does not distinguish them. An
    enterprise approval of `intent_hash = "aaaa…"` — sixty-four characters I chose — is
    accepted, MAC'd and returned in the shape a verifier reads as an RS-011 binding. It is an
    RS-013 attribution (an attested identity said something) wearing an RS-011 binding's
    clothes.

    After this change the ref states which one it is, and a conformance claim cannot equate
    them.

  in_scope:
    - "The approval ref states the provenance of its intent hash: derived by the re-deriving engine, or asserted by the caller"
    - "The field is required, with no default that would let an emitter stay silent"
    - "The OSS engine emits derived; the enterprise emits asserted"
    - "Ingest and the console surface it, and any claim or conformance statement distinguishes the two"
    - "RS-011's text says which provenance satisfies it"

  out_of_scope:
    - "Making the enterprise derive an intent (it has no engine; that is a different project)"
    - "Refusing asserted approvals — an attested approval is evidence, just not this kind"
    - "The unbound case, which is #444 and emits no ref at all"

  affected_layers:
    domain:
      - "Modified: ApprovalRecordRef gains the provenance of its intent hash"
    application:
      - "Modified: envelope factory emits derived; enterprise emits asserted"
    infrastructure:
      - "None"
    api:
      - "Modified: envelope schema, SDK contract, console/claims surfaces"

  canonical_references:
    - module: ".pi/architecture/modules/audit.md#envelope"
    - adr: ".pi/architecture/decisions/ADR-011-approval-binding.md"
    - last_sync: "2026-10-31"
    - evidence: "engine/src/audit/domain/envelope.rs:290 — ApprovalRecordRef: node_id, step_name, intent_hash, approver_id, authority, decided_at; no provenance"
    - evidence: "rigorix-sdk schemas/envelope.json — approvalRecordRef requires intent_hash, described as 'intent hash the approval is bound to'"
    - evidence: "docs/spec/execution-governance/03-requirements.md:40-49 — RS-010/RS-011/RS-012 require a canonical intent and a pre-dispatch re-derivation"
    - evidence: "live, armed enterprise: intent_hash of 64 'a' accepted, MAC returned; no field distinguishes it from a derived one"

  acceptance_criteria:
    - "The ref cannot be constructed without stating whether the intent was derived or asserted"
    - "The OSS engine's refs are always derived, and a test asserts it"
    - "The enterprise's refs are always asserted, and a test asserts it"
    - "Ingest preserves the distinction and the console reports it per record"
    - "The SDK schema, its description and the spec's RS-011 name the two and say which satisfies the requirement"
    - "A conformance claim citing approval binding states the provenance of the evidence it cites"

  validators:
    - ci
    - tests
    - canonical
    - docs

  implementation_notes: |
    - The precedent is `HistoryIntegrity`: three values where only one asserts authenticity.
      Two approvals that differ in kind must not share a representation for the same reason.
    - The field must be required, not `Option`. A default is how the difference disappeared in
      the first place.
    - This is not a criticism of the enterprise design: an L2 deployment can attest who said
      what, and the compose file already states that ceiling. The artifact is what fails to
      say it, and the artifact outlives the compose file.
    - Do not solve it by refusing asserted refs at ingest. A refused approval is a lost
      decision; a labelled one is evidence about a weaker thing.

  file_changes:
    - "modify: engine/src/audit/domain/envelope.rs"
    - "modify: engine/src/audit/application/envelope_factory_impl.rs"
    - "modify: docs/spec/execution-governance/03-requirements.md"
    - "modify: docs/spec/execution-governance/05-evidence-model.md"
    - "modify: rigorix-sdk schemas/envelope.json"
    - "create: engine/tests/unit/audit/approvalref/approvalref_provenance_test.rs"

  pipeline_steps:
    - implement
    - validate
    - create-mr
    - merge
---

# ISSUE-APPROVAL-REF-CANNOT-SAY-DERIVED-OR-ASSERTED

## The two facts one field carries

```
OSS engine:       H(run_key, canonical(tool ‖ resolved payload ‖ declared scope))   derived
enterprise L2:    a string the caller sent, MAC'd over "approver said this"          asserted
```

RS-011 asks for the first. RS-012 requires the re-derivation that produces it. The second
satisfies RS-013 (attested identity) and nothing more — and it is emitted in the identical
shape, verifies identically, and reads identically.

Measured, not argued: in an armed enterprise deployment,

```
rigorix.approve { node_id: 4a111fab-…, step_name: "deploy-prod",
                  intent_hash: "aaaaaaaa…" (64 chars, chosen) }
→ approval_ref { node_id: 4a111fab-…, step_name: "deploy-prod",
                 intent_hash: "aaaa…", mac: "2f68206f…" }
```

The MAC proves the approver said that hash. It cannot prove the hash is that node's intent,
because the enterprise never resolved one.

## Why the artifact must say it

An envelope is read by people who never see this repository's compose file, and often years
later. `intent_hash` present + `mac` present = "bound to a canonical intent" is exactly the
reading RS-011 licenses, and the enterprise deployment would produce it while being unable to
perform RS-012 at all.

The repo already made this call for history integrity: three values, and only one of them
(`anchored`) asserts authenticity. Two approvals of different kinds must not share one
representation.

## The required end state

The ref states whether the intent hash was derived or asserted, the field is required, the
engine emits `derived`, the enterprise emits `asserted`, ingest and the console surface it,
and RS-011 says which one satisfies it. A conformance claim citing approval binding states
the provenance of the evidence it cites.

## How it is proven

| seed | expected |
|---|---|
| OSS run with an approved step | ref provenance `derived` |
| enterprise approve (armed) | ref provenance `asserted` |
| a ref constructed without provenance | compile error, not a default |
| ingest of an envelope carrying both kinds | both preserved, console shows each |
| a claim citing approval binding while only asserted evidence exists | the claim audit refuses it |
