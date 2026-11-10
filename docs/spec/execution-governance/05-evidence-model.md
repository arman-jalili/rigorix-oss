# 05 — Evidence model

**Status: normative.** This document defines what evidence exists, what each
property proves, and what a verifier may conclude.

---

## 1. Why this document exists

Most audit claims fail here. "Signed", "chained", "tamper-evident", "anchored" and
"immutable" are used interchangeably. They are not the same property, and a
verifier that conflates them can be made to accept a forged history.

This document separates four properties and defines a verification level for each.

## 2. The artifacts

| Artifact | Contents | Produced by |
|---|---|---|
| Decision record | rule id, step, outcome, inputs consulted, digests, time | gate |
| Envelope | execution/step event, identity, timing, outcome, findings, integrity metadata | evidence writer |
| Chain | envelopes linked by `prev_hash` and `sequence` | evidence writer |
| Receipt | signed binding of a history state to identity and time | anchor |
| History slice | signed range of the history, for offline verification | anchor |
| Checkpoint | signed state at a sequence, used for compaction | anchor |

### The decision record's `time`

The `time` of a decision record is **the instant the decision was evaluated with** —
not the instant at which the record was written, and not the wall clock at
verification time. With pinned inputs it may be supplied by the caller and is signed
with them (ADR-014 §2), so the instant is part of the evidence rather than a property
of the machine that happened to run the gate.

A verifier replays a decision with the recorded instant. Substituting the wall clock
makes the replay a new decision with new inputs, and any difference it reports is a
property of the verifier's clock rather than of the record — which is why replay with
**any** recorded instant is normative and replay with the current instant is not.

### Refused is a decision; unavailable is not

The distinction is normative wherever a decision is recorded:

- A **refusal** is a decision. The gate evaluated the inputs and concluded that the
  step must not proceed. It carries a refusal reason, and the evidence records it as
  an outcome (see `01-terminology.md`, *Refusal*).
- An **unavailable evaluation** is the absence of a decision. The gate could not
  evaluate — the engine was not linked, a port could not answer, the inputs were
  unparseable. It MUST NOT be recorded as a refusal, and a host MUST NOT be able to
  read it as an allow either.

Both fail closed, and that is the point of keeping them apart: a refusal says the
system decided, an unavailable evaluation says the system did not. Collapsing the two
loses the only fact an auditor needs to tell a policy that fired from a gate that was
not there.

## 3. The four properties

**P1 — Internal consistency.** The envelopes form a valid chain: hashes link,
sequences are contiguous, no gaps. Establishes: nothing was removed or reordered
*within the set you were given*.

**P2 — Integrity against a key.** Each envelope carries a MAC over its canonical
form. Establishes: the record was produced by a holder of that key, and was not
modified after signing. Requires the verifier to hold the key; if the key is held by
the party being audited, P2 is not independent.

**P3 — External provenance.** A receipt, slice or checkpoint is signed by a key the
producer does not control at verification time. Establishes: this specific history
state existed and was attested by that key holder. This is the only property that
survives a compromised producer.

**P4 — Attribution.** The identity in the record is attested (device flow, IdP,
workload identity), not asserted. Establishes: a specific subject performed the
action, to the strength of the attestation.

A record can satisfy P1 and not P2 (unsigned). It can satisfy P1 and P2 and not P3
(self-signed chain). It can satisfy P1–P3 and not P4 (anonymous signer).

### Approval evidence: binding vs attribution

An approval's `intent_hash` does not say, by itself, what it proves. The ref's
required `intent_hash_provenance` does:

| Value | Meaning | Satisfies |
|---|---|---|
| `derived` | The re-deriving engine computed the hash from the sealed graph (tool, resolved payload, declared scope). | RS-011 binding; RS-012 can re-derive and halt on mismatch. |
| `asserted` | A caller supplied the hash; an attesting service recorded that the approver said it. | RS-013 attribution only. It does **not** satisfy RS-011, because no engine resolved the intent. |

Both are evidence; they are not the same evidence. A conformance claim citing
approval binding MUST state the provenance of the refs it cites and MUST NOT cite
`asserted` refs as proof of RS-011.

## 4. Verification levels

| Level | Name | Requires | A verifier may conclude |
|---|---|---|---|
| V0 | Unverified | nothing | the file parses |
| V1 | Internally consistent | envelopes only | the supplied set is self-consistent; a removed or reordered record within the set is detectable |
| V2 | Integrity-checked | envelopes + key | the set was produced by a holder of the key and unmodified since |
| V3 | Externally proven | envelopes + receipt/slice/checkpoint + public key | this history state was attested by the anchor key holder at that time |

**Conformance requirement RS-064:** a verifier MUST report which level it achieved,
and MUST NOT describe V1 or V2 as proving external provenance.

## 5. What each level does not prove

| Level | Does not prove |
|---|---|
| V1 | that the set is complete relative to what the producer actually did; a producer can present a self-consistent subset |
| V2 | independence, if the key belongs to the audited party |
| V3 | that the underlying action was legitimate, or that the authority data was true |
| Any | that the decision was correct — only that it was made, recorded and (at V3) attested |

## 5a. Completeness is a status on the run (0.4.0)

A record answers "what was decided". A **run** answers a second question: "did every
decision in it reach the record". RS-105 makes that a status rather than a log line:

| Status | Meaning | What a host must do |
|---|---|---|
| `complete` | every decision in the run has a record that was emitted, and delivered where delivery is configured | nothing |
| `degraded` | the run completed, but at least one record was not emitted or not delivered | print the reason and the count; never report the run as complete |
| `not_requested` | evidence was not configured for this run | say so, once |

Two properties follow, and both were defects before they were requirements:

1. **A dropped result is not a policy.** The evidence path computes a result at every
   step (build, persist, deliver, enqueue). A result that can be discarded silently makes
   the difference between "the evidence exists" and "we ran the code" unobservable. The
   implementation marks those results `#[must_use]` and a dropped one fails the build.
2. **A queue that loses its contents on restart is not a retry queue.** If undelivered
   envelopes live only in memory, `degraded` is the honest status until they are
   persisted; the loss window must be stated, not implied.

The operand discipline of Consequence Gating v2.0 §2.7 applies here too: the reason and
the count are fields. A sentence that interpolated them would put a deployment's numbers
into a static string.

## 6. Degraded evidence

An envelope that lacks a signature, an anchor reference, or a required field MUST be
marked degraded. Degradation is a property of the record, not a verdict on the run.
A deployment that produces degraded evidence is not necessarily non-conforming; a
deployment that presents degraded evidence as intact is (RS-061).

## 7. Offline verification procedure

1. Parse envelopes; check schema version.
2. Recompute each envelope's canonical form and MAC (V2).
3. Recompute the chain: `prev_hash`, `sequence`, continuity (V1).
4. If a receipt/slice/checkpoint is supplied, verify its signature over the canonical
   form with the anchor public key (V3).
5. Compare the attested state (sequence, head hash, time) to the chain you verified.
   A mismatch is a hard failure.
6. Report: level achieved, artifacts used, and any property not established.

Step 6 is mandatory. A verifier that prints "verified" without naming what it
verified has produced a false assurance.

## 8. Canonical form

Signatures are over a canonical serialisation, not over the JSON as transmitted. The
canonical form MUST be specified and versioned (key ordering, number format, unicode
normalisation, absent-vs-null). Two implementations that disagree on canonical form
will disagree on verification, and each will believe the other is tampered with.

## 9. Anchor trust

The anchor key is the root of P3. Its publication, rotation and revocation are
outside this specification, but a deployment claiming V3 MUST document:

- who holds the anchor key,
- where the public key is published,
- how a verifier obtains it,
- what happens when it rotates.

An anchor whose key is held by the same party that produced the history provides P2
with extra steps.

## 10. Reference implementation

- Envelope schema v2 and signed fixtures: the `rigorix-schemas` crate (and the
  verifier packages' bundled copy).
- Verifiers: `rigorix-verifier` 0.9.0 on crates.io, npm and PyPI; Java and Go in the
  same tree as this document. The Python package verifies chain, signature and anchor
  artifacts but has no effect-commitment verification — see `10-non-claims.md` §3 item 14
  before citing PyPI as a channel for a commitment claim.
- Functions: `verify_chain`, `verify_chain_with_checkpoints`, `verify_ed25519`,
  `verify_ledger_receipt`, `verify_history_slice`, `verify_compaction_checkpoint`.
- Anchor: Ed25519 signed reads in the enterprise audit service (ADR-016).


## 11. The effect record

The commitment context adds a fifth artifact:

| Artifact | Contents | Produced by |
|---|---|---|
| Effect record | intent key, state (`Dispatched`/`Committed`/`NotCommitted`/`Ambiguous`), observed outcome, resolver identity, transition log | effect-commitment context |

It answers a question the envelope cannot: *was this effect committed?* An envelope
records what the engine did. An effect record records what became of it.

## 12. Commitment verification

| What is checked | What it establishes |
|---|---|
| The record exists in `Dispatched` before the effect was sent | the window was entered deliberately |
| State transitions are legal and logged with actor and reason | the record was not rewritten silently |
| A `Committed` state carries a provider receipt with an evidence digest | settlement, not opinion |
| An `Ambiguous` state has no resolver, or a resolver with evidence | nobody settled it by guessing |

A verifier MUST NOT treat the absence of a record as `NotCommitted`. Absence is
either "never attempted" or "not recorded", and the two are different claims.

## 13. The two-axis claim

Commitment claims are a cell, not a level (see `02-conceptual-model.md` §12). A
verifier or a deployment statement that says "L3" without naming durability is
incomplete. The `rigorix doctor` output prints the derived cell.
