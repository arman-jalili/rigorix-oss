# ADR-021: The signed record carries a subject handle, and the human behind it is erasable

**Status:** Proposed (under review). **Not accepted, not scheduled.** The mechanism is decided here;
the legal question in § Open questions is not, and this ADR does not pretend to answer it.
**Date:** 2026-11-05
**Supersedes:** nothing. Amends ADR-011 (approval binding) by naming what the binding records about
the approver, and completes the item ADR-020 § D7 deferred: *"the pseudonym/erasure decision moves to
ADR-021 with the legal question undecided."*
**Superseded by:** nothing.
**Related:** ADR-016 (chained log and its boundary), ADR-019 (effect commitment — `claim.subject()`
is the vocabulary this reuses), ADR-020 (approval store; Proposed), ADR-011 (approval binding),
ADR-009 (error handling — redaction at construction).

## Context

An approval is evidence that an authorised human decided. Two facts about how that evidence is built
today pull in opposite directions.

### What the code does today

| fact | citation |
|---|---|
| The approval record stores a human identifier as a free string | `engine/src/approval/domain/record.rs:154` `pub approver_id: String` |
| The same string is copied into the signed envelope, so it is part of the canonical bytes and of the MAC chain | `engine/src/audit/domain/envelope.rs:316` `ApprovalRecordRef::approver_id` |
| It is materialised from a caller-supplied payload | `engine/src/audit/application/envelope_factory_impl.rs:733-749` (`payload.get("approver_id")`) |
| The engine's own example fixture uses an email address as that value | `engine/src/audit/application/envelope_factory_impl.rs:1055` `"approver_id": "user@org"` |
| Binding mode refuses an approval that names no approver | `engine/src/execution_engine/tests/approval.rs` `test_approve_without_approver_id_is_refused_in_binding_mode` |
| Scope is recorded beside it | `engine/src/approval/domain/record.rs:173` `pub approver_scope: Vec<String>` |
| Enterprise stores the same identifier in a column, keyed by claim | `core/migrations/046_approval_decision_binding.sql:18`, `core/src/execution_api/application/services.rs:257` (`"approver_id": decision.claim().subject()`) |

### The tension, stated plainly

A signed record cannot be edited: that is what makes it evidence (ADR-016). A personal identifier
inside one therefore cannot be erased by editing, and erasing it by breaking the chain destroys the
property the record exists to carry. Today the design has no third option, so every erasure request
has exactly two possible answers, both wrong: *rewrite signed bytes* or *refuse*.

The same repository already holds the vocabulary for the third option. An effect commitment carries a
**subject** — an opaque handle — and the enterprise approval path already writes
`decision.claim().subject()` into the column that is named after the person (services.rs:257). The
identification of a subject with a human is exactly the fact that must be able to disappear.

## Decision

**D1 — The signed record carries an opaque `approver_subject`, never an identifier.**
`ApprovalRecordRef` gains `approver_subject: String` and loses `approver_id`. The handle is a
high-entropy public value (a random 128-bit id rendered as text), not a hash of an identifier and not
an email. A hash is rejected in § Rejected alternatives: an email's space is small enough to enumerate.

**D2 — The handle is assigned by the approving surface, not by the caller.**
A caller may claim any string; only the surface that authenticated the human may say who the subject
is. This follows the enterprise precedent that *dispatch identity is the credential, not the body*:
the subject is derived from the authenticated principal at approval time and cannot be supplied in
the request payload.

**D3 — The mapping from subject to human is a separate, erasable store, outside every signed byte.**
A table (`approval_subject` — subject, tenant, human identifier, created_at, erased_at) lives beside
the approval store, is never an input to canonical bytes, and is the only place a name appears.
Deleting a row is a legal operation that no signature covers.

**D4 — Erasure is an event, not an edit.**
Erasing a subject writes `subject_erased { subject, at, reason_code }` to the erasure ledger, sealed
by the same MAC discipline as the consumption ledger (ADR-011 § ledger), and then removes the
mapping. A verifier that finds no mapping can therefore distinguish *intentionally erased* from
*never known* — which is the difference between a lawful erasure and a gap in the evidence.

**D5 — Authority survives the identity.**
What a verifier needs after erasure is not a name, it is a claim: *this decision was approved by
someone entitled to approve it*. The signed record therefore carries the authority as a claim
(`authority`, `approver_scope`, plus the role id), and the erasure of a subject removes attribution,
not authorisation. A verifier can answer "was this approved within policy" with no personal data at
all; only "by whom" becomes unanswerable, and by design.

**D6 — Mixed vocabularies are named, not migrated.**
Records already signed with `approver_id` cannot be rewritten. The envelope gains a
three-value marker, in the shape ADR-011 already uses for `intent_hash_provenance`:

| value | meaning |
|---|---|
| `subject` | an opaque handle; the mapping is (or was) in the erasable store |
| `literal` | a pre-ADR identifier, present in the signed bytes and not erasable by this mechanism |
| `absent` | no approver recorded; only admissible where binding mode is not required |

A reader can therefore tell what it is looking at without trusting a version number, and no migration
of signed history is attempted.

**D7 — Redaction happens at construction, not at display.**
The raw identifier must never enter an error string, a log line, a span, or an evidence summary. The
precedents are `redacted_path` and `endpoint_redacted`: the value is reduced where the object is
built, because a redaction applied at render time is one code path away from being forgotten.

**D8 — The enterprise column keeps its name and changes its meaning.**
`approval_decisions.approver_id` already stores `claim().subject()` (services.rs:257); it becomes
`approver_subject` in the next migration, and the console displays a subject id with the resolved name
**only** when the mapping exists and the reader is entitled to it. A console that cannot resolve a
subject shows the handle and says the mapping was erased, rather than showing a blank that looks like
a defect.

**D9 — No crypto-shredding.**
Encrypting the identifier and deleting the key is rejected: availability of the entire verification
path then depends on one key's custody, and a lost or rotated key destroys evidence that was
otherwise intact. The evidence must remain verifiable by a party who never had the mapping.

## Rejected alternatives

| | why not |
|---|---|
| **Record the identifier and answer erasure requests with "signed, therefore permanent"** | It is a permanent personal-data store whose lawful basis nobody designed, and it makes the product unusable in the one market that asks the question first. |
| **Crypto-shred: encrypt the identifier, delete the key on request** | Ties the availability of evidence to one key's custody (D9). A backup of the key resurrects the data; a lost key destroys the record. |
| **Hash the identifier in place of a handle** | An email addresses have a small enumerable space; a hash is reversible by guessing. That is pseudonymity by assumption, which is the failure mode this project keeps naming. |
| **Drop the approver field entirely** | ADR-011 requires that a dispatch be bound to an approver; an approval with no subject is indistinguishable from no approval at all. |
| **Rewrite signed records during migration** | Destroys the property the record exists for. Named here so it is not proposed again. |

## Consequences

**Positive.** Evidence survives erasure and keeps verifying. The OSS and enterprise vocabularies
converge on one noun (`subject`) that already exists in ADR-019. A verifier can answer the policy
question without personal data. An erasure request becomes a supported operation with an audit trail
instead of an exception handled in a support thread.

**Negative.** A second store to secure, and it is a high-value target: it is the only place the
mapping exists, so its access log matters as much as its contents. Records signed before this ADR
carry a vocabulary that cannot be erased, and the product has to say so in one sentence rather than
imply uniform behaviour. Two identifiers now exist in the system (subject, and the human in the
mapping), so every display path must state which one it is showing.

**Cost if adopted later rather than now.** Every approval signed from today carries a literal
identifier into an immutable chain. The count of those records only grows, and each one is a request
this mechanism cannot answer.

## Open questions (for the operator, not for the implementer)

1. **What is the retention basis for the mapping?** The law is jurisdictional; the mechanism in D3/D4
   can express any policy, including none. This ADR does not choose one.
2. **Who may read the mapping?** A console that resolves subjects is a re-identification tool. The
   answer is a permission and an audit, not a default.
3. **Is `absent` ever admissible?** D6 permits it where binding mode is off. Whether a deployment may
   run in that mode is a product decision, not an engineering one.
4. **Does the erasure ledger itself need an anchor?** ADR-016's anchor covers the audit chain; an
   erasure ledger outside it is a second chain whose truncation detection is an open question.

## Adoption triggers

Adopt when any of these becomes true, whichever is first:

- an erasure request arrives for an approval in a signed record;
- a new approval-capable surface ships (a second host, a second console);
- the enterprise console displays an approver name to a reader who is not the approver;
- the pseudonym is proposed for a second context (capacity ledger, budget authorisation), which would
  make it a shared vocabulary rather than an approval detail.

Until one does, this ADR stays Proposed and no code changes. That is deliberate: it is a decision
about what the evidence may contain, and it should be reviewed before the first record depends on it.
