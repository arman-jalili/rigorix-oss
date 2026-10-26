# Rigorix Specification — Execution Governance

**Version:** 0.2.0 (public review draft)
**Status:** Published for review and quotation. Cite it, quote it, and disagree with it
in public — that is what a review draft is for. Two limits remain: no conformance
claim may be made against a draft version (see `09-conformance.md` §7), and a
published version is immutable, so corrections arrive as a new patch version
(`12-versioning.md` §5).
**Date:** 2026-10-25

---

## What this document states, and for whom

This specification defines **execution governance** as Rigorix practises it: deciding
again at the moment an action becomes an effect, recording every decision, and owning the
effect until it is settled.

It was written from the implementation — the engine, the verifiers, the deployments in
`11-traceability.md` — and it is the source of its own vocabulary. The terms, the
requirement set, the evidence model and the conformance levels were derived from what the
system does and from the failures it was built to prevent, not adopted from another
framework.

The requirements are normative for a system that claims conformance to this
specification, and for nothing else. Requirement identifiers are `RS-NNN` so a citation
is unambiguous.

## What this is

This is the specification for **execution governance**: the layer that decides
whether a consequential AI-agent action may happen, at the moment it happens, and
produces evidence a third party can verify.

It defines the terminology, the conceptual model, the normative requirements, the
evidence model, the threat model, the conformance levels, and the explicit
non-claims. It also covers the **unsettled interval** after a consequence is
dispatched: an effect whose outcome is unknown is a state, not a guess (§J of
`03-requirements.md`, `02-conceptual-model.md` §11–12). It is implementation-neutral:
Rigorix is the reference implementation, not the definition.

## How to read this

Three reading paths. Pick the one that matches why you are here.

**If you are evaluating the category** (CISO, CTO, risk, analyst)
→ `00-scope.md` → `02-conceptual-model.md` → `07-threat-model.md` → `08-applicability.md` → `10-non-claims.md`

**If you are implementing against it** (engineer, platform, integrator)
→ `01-terminology.md` → `02-conceptual-model.md` → `03-requirements.md` → `04-architecture.md` → `05-evidence-model.md` → `09-conformance.md`

**If you are auditing a claim** (auditor, reviewer, counterparty)
→ `03-requirements.md` → `05-evidence-model.md` → `09-conformance.md` → `11-traceability.md` → `10-non-claims.md`

**If you are deciding what to do when an effect's outcome is unknown**
→ `02-conceptual-model.md` §11–12 → `03-requirements.md` §J → `05-evidence-model.md` §11–13

## Document map

| Document | Purpose | Kind |
|---|---|---|
| `00-scope.md` | Purpose, audience, scope, non-goals, normative language, versioning | Normative framing |
| `01-terminology.md` | The controlled vocabulary. Definitions, terms to avoid, anchors | **Normative** |
| `02-conceptual-model.md` | Entities, lifecycle, three moments, gate sequence, state | **Normative** |
| `03-requirements.md` | Numbered requirements (RS-001…), each with rationale and trace | **Normative** |
| `04-architecture.md` | Layers, components, deployment modes, boundaries | Informative |
| `05-evidence-model.md` | Envelope, chain, anchor, receipts, verification levels | **Normative** |
| `06-actors.md` | Roles, responsibilities, separation of duties | Informative |
| `07-threat-model.md` | Adversaries, attack classes, controls, residual risk | Informative |
| `08-applicability.md` | Domain classes the model applies to, with example controls | Informative |
| `09-conformance.md` | Conformance levels, claim format, fixtures, self-test | **Normative** |
| `10-non-claims.md` | What is explicitly not claimed | **Normative** |
| `11-traceability.md` | Requirement → implementation → test → demo | Informative |
| `12-versioning.md` | Version policy, change classes, deprecation | Normative framing |

## Precedence

When documents disagree, the higher row wins:

1. `03-requirements.md` (the numbered obligations)
2. `01-terminology.md` (definitions constrain interpretation)
3. `05-evidence-model.md` and `09-conformance.md`
4. `10-non-claims.md` (a claim that contradicts this is void)
5. Informative documents (`02`, `04`, `06`, `07`, `08`, `11`)
6. The reference implementation and its tests

A reference implementation never creates an obligation. It is evidence that an
obligation can be met.

## Normative language

MUST, MUST NOT, SHOULD, SHOULD NOT, MAY are used as defined in RFC 2119 and
RFC 8174, and only in capitals.

Each requirement has an identifier (`RS-NNN`) so it can be cited, tested and
traced. Identifiers are stable. Withdrawn requirements keep their number and are
marked withdrawn.

## Design principles behind the requirements

1. **Decide at the moment of consequence.** A decision made only at plan time or
   approval time describes a world that may no longer exist.
2. **The plan is the artifact.** The model proposes; the plan is what is reviewed
   and enforced.
3. **Composition is the unit.** Individually permitted actions can be collectively
   wrong. Constraints apply across steps, runs and sessions.
4. **Determinism in the decision path.** No model judgment inside enforcement.
5. **Evidence is a product.** A decision without verifiable evidence is an opinion.
6. **State the boundary.** A control that cannot state its limits cannot be
   audited, and cannot be trusted.
7. **Fail closed on ambiguity.** Unknown is not permission.

## Relationship to Rigorix

Rigorix is the reference implementation:

- engine: plan/DAG execution, gate sequence, approval binding, preconditions,
  sequence policy, effect keys, capacity ledger, audit chain
- SDK: schemas and verifiers in five languages
- enterprise: policy, ingest, ledger, console, compliance mapping

Where this specification and the implementation disagree, the specification is
wrong or the implementation has a defect. Both are fixable; the disagreement must
be recorded.

## Status of conformance claims

No conformance claim may be made against this draft. A claim requires a published
version, a declared conformance level, and the evidence listed in
`09-conformance.md`.
