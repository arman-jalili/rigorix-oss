# 09 — Conformance

**Status: normative.**

---

## 1. What a conformance claim is

A claim that an implementation satisfies a set of requirements at a declared level,
in a declared deployment mode, with stated exceptions.

A claim is a statement of evidence, not of intent. "We are designed to" is not a
claim.

## 2. Levels

Levels are cumulative. An implementation declares the highest level it satisfies.

### Level 1 — Core execution governance
Requirements: EG-001–EG-005, EG-010–EG-014, EG-050–EG-054, EG-060–EG-062, EG-070–EG-073.
Capability: a consequential action is expressed as a plan, gates run at a single
choke point, approvals bind to intent, decisions are recorded, refusals prevent
dispatch.

### Level 2 — Composition
Adds: EG-030–EG-035.
Capability: sequence constraints and effect identity across steps and runs, including
cross-run evaluation over signed history.

### Level 3 — Commit-time authority
Adds: EG-020–EG-026.
Capability: operator checks re-evaluated at dispatch against live authority, with
fail-closed semantics and signed findings.

### Level 4 — Verifiable evidence
Adds: EG-063–EG-066.
Capability: chain, signature and external anchor, with offline verification that
reports its achieved verification level (V1–V3).

### Level 5 — Distributed capacity
Adds: EG-040–EG-043.
Capability: aggregate exposure enforced with reserve-then-authorize against a
contested store, across sessions and replicas.

### Level 6 — Commitment
Adds: EG-090–EG-098.
Capability: the unsettled interval is owned. Write-ahead record, non-terminal
`Ambiguous`, dependents fenced, retries refused, resolver authority enforced.
Level 6 requires a declared durability axis (L1 or L2) and a declared settlement
axis (none or L3); the two are independent.

## 2a. Commitment claim matrix

A Level 6 claim MUST name one cell:

| Durability | Settlement | What the deployment can say |
|---|---|---|
| L1 in-process | none | "we know when an effect is unsettled; the record dies with the process" |
| L1 in-process | L3 receipt | "we can settle, but a restart loses the record" |
| L2 contested store | none | "the record survives and is shared; we cannot prove commitment" |
| L2 contested store | L3 receipt | "we can prove commitment, and the record survives" |

## 3. Claim format

```
Implementation: <name and version>
Specification:  execution-governance 0.1
Level:          <1-5>
Deployment:     engine-only | engine+verifier | engine+control-plane
Anchor holder:  <party or "none">
Durability:     L1 in-process | L2 contested store
Settlement:     none | L3 provider receipt
Not met:        <requirement ids, or "none">
Evidence:       <links: test run, fixtures verified, demo output>
Date:           <date>
```

A claim without the `Not met` field is incomplete. Every real deployment has
exceptions, and stating them is what makes the rest credible.

## 4. Evidence required per level

| Level | Minimum evidence |
|---|---|
| 1 | a run showing plan, gate, approval binding, refusal, decision record |
| 2 | a run showing a sequence constraint promoting or refusing, including a cross-run case |
| 3 | a run showing a precondition refusing after the world changed, with a signed finding |
| 4 | an offline verification at V2 and V3, with the verifier's own level report |
| 5 | a concurrency test where a naive counter breaches and reservation holds |

Fixtures SHOULD be published so a third party can reproduce them.

## 5. Self-test procedure

1. Run the Level 1 scenario on a clean machine. Confirm the decision record exists and
   names the rule that fired.
2. Change the world between approval and dispatch. Confirm refusal and a signed
   finding with check and authority digests.
3. Split a prohibited sequence across two runs. Confirm the second is refused from
   history.
4. Verify the envelope offline at V2. Then verify a receipt at V3 and confirm the
   verifier reports which level it achieved.
5. Run the capacity scenario. Confirm the naive path breaches and the reservation
   path holds, and that the in-process mode is not described as cross-session.
6. Attempt the bypasses: compound shell string, direct node API, edit a rule from the
   agent's session. All must fail.

Step 6 is the one most implementations skip and the one that matters most.

## 6. Partial conformance

Partial conformance is allowed and must be expressed as exceptions. Example: an
implementation may satisfy Level 4 but not EG-063 if it has no external anchor; it
then declares Level 3 with a note that chain and signature exist but no anchor.

What is not allowed is declaring Level 4 while the anchor is self-held and calling it
external provenance (EG-064, EG-081).

## 7. Conformance of this specification itself

This specification is a draft. No claim may cite it until version 1.0 is published.
Draft conformance exists only for internal review.
