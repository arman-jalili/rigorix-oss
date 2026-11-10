# 09 — Conformance

**Status: normative.**

---

## 1. What a conformance claim is

A claim that an implementation satisfies a set of requirements at a declared level,
in a declared deployment mode, with stated exceptions.

A claim is a statement of evidence, not of intent. "We are designed to" is not a
claim.

## 1a. Deployment modes

A deployment mode is the set of capabilities an implementation ships. Two modes are
defined here because the difference changes what a claim may say:

- **Producer** — evaluates, decides, and writes signed evidence. It may ship without
  any verifier, as the wasm build does (ADR-013): the artifact produces evidence and
  cannot perform offline verification of it.
- **Verifier** — independently verifies evidence produced elsewhere: chain integrity,
  signatures, anchors, and replay of the decision inputs.

A producer-only deployment MUST state its mode in the claim and MUST NOT claim a
level whose evidence requires independent verification — that is Level 4 (verifiable
evidence) and above. It claims the levels its production evidence actually satisfies,
with the verification rows marked *excepted, producer-only mode* (see §6).

The obligation is not that every deployment verify. It is that a claim names which
side of the boundary the deployment is on: evidence that nobody can verify is not
verifiable evidence, and a claim that conflates the two is the claim `10-non-claims.md`
refuses.

## 1b. The in-process binding (0.4.0)

A third *form* of deployment, not a third mode: the gate as an artifact the host loads
(ADR-022). It is a producer — it evaluates and returns a decision and
it cannot verify anything — and a claim about it must state three further facts:

1. **the digest it loaded**, with the platform and commit the digest belongs to (RS-106);
2. **the embedder duties it performed**, because the import table bounds what the module
   *can* reach and not what the host *grants* (§4.4 there; RS-103);
3. **the mode it is not**: an artifact cannot verify evidence, so a deployment that is
   only an artifact cannot claim Level 4 and above.

A claim that cites "the module imports six functions and no decision reads them" is a
statement about the recorded vectors under the recorded digest. It is not a statement
about a particular host, and a claim that conflates the two is what §1a already forbids
for producer-only deployments.

## 2. Levels

Levels are cumulative. An implementation declares the highest level it satisfies.

### Level 1 — Core execution governance
Requirements: RS-001–RS-005, RS-010–RS-014, RS-050–RS-054, RS-060–RS-062, RS-070–RS-073.
Capability: a consequential action is expressed as a plan, gates run at a single
choke point, approvals bind to intent, decisions are recorded, refusals prevent
dispatch.

### Level 2 — Composition
Adds: RS-030–RS-035.
Capability: sequence constraints and effect identity across steps and runs, including
cross-run evaluation over signed history.

### Level 3 — Commit-time authority
Adds: RS-020–RS-026.
Capability: operator checks re-evaluated at dispatch against live authority, with
fail-closed semantics and signed findings.

### Level 4 — Verifiable evidence
Adds: RS-063–RS-066.
Capability: chain, signature and external anchor, with offline verification that
reports its achieved verification level (V1–V3).

### Level 5 — Distributed capacity
Adds: RS-040–RS-043.
Capability: aggregate exposure enforced with reserve-then-authorize against a
contested store, across sessions and replicas.

### Level 6 — Commitment
Adds: RS-090–RS-098.
Capability: the unsettled interval is owned. Write-ahead record, non-terminal
`Ambiguous`, dependents fenced, retries refused, resolver authority enforced.
Level 6 requires a declared durability axis (L1 or L2) and a declared settlement
axis (none or L3); the two are independent.

**Claim discipline (RS-080–RS-082) applies at every level** and is therefore not
listed under any one of them: a deployment at any level that states a guarantee must
state the level, the mode and the requirements it does not meet.

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
Specification:  execution-governance 0.2
Level:          <1-6>
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
| 4 | an offline verification at V2 and V3, with the verifier's own level report — **not satisfiable by a producer-only deployment**, which by §1a ships no verifier and MUST NOT claim this level |
| 5 | a concurrency test where a naive counter breaches and reservation holds |
| 6 | a run where a dispatched effect is left unsettled: the retry is refused, a dependent is fenced, and a resolver settles it with a provider receipt — with the durability and settlement axes named |

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
implementation may satisfy Level 4 but not RS-063 if it has no external anchor; it
then declares Level 3 with a note that chain and signature exist but no anchor.

What is not allowed is declaring Level 4 while the anchor is self-held and calling it
external provenance (RS-064, RS-081).

A producer-only deployment (§1a) records the verification rows as
*excepted, producer-only mode* rather than omitting them: the exception names the
capability it does not ship, so a reader can see that the deployment produces
verifiable evidence without claiming that this deployment verified it.

## 7. Conformance of this specification itself

This specification is a draft. No claim may cite it until version 1.0 is published.
Draft conformance exists only for internal review.
