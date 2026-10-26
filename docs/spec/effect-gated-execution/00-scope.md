# 00 — Scope, audience, and normative framing

## 1. Purpose

This specification defines **effect-gated execution** for AI-agent and automated
systems that perform consequential actions — the discipline of deciding again at the
moment an action becomes an effect, recording the decision, and owning the effect
until it is settled.

It exists because a class of control is missing from current practice: a
deterministic decision made at the moment an action becomes real, with evidence
that a third party can verify afterwards.

**On the name.** "Execution governance" is in use as a category name by other authors
(see `README.md`). This document no longer uses it as its own, and defines no
framework but the one below. The rename is recorded in `12-versioning.md` §7.

## 2. The problem, stated once

Authorization is normally decided before an action executes and is then treated as
a property of the action. That works when the decision and the effect happen in the
same moment and the world holds still.

Agents separate the decision from the effect. They plan, wait for approval, resume
later, run in parallel, and operate across sessions. In the gap, authority changes:
a beneficiary is frozen, a limit is lowered, a credential is revoked, a schema
constraint appears, a capacity budget is consumed by a sibling session.

A second class of failure does not involve time at all. Several individually
permitted actions compose into an outcome no single decision was permitted to
reach. The composition never appears in any single decision's context.

A third does not involve permission at all. The effect is dispatched and the outcome
is unknown — the response was lost, the process died, the deadline passed. A
two-state world records that unknown as a lie with a timestamp.

Effect-gated execution addresses all three: the decision is made again at the moment
of consequence, constraints apply to composition rather than to single requests, and
the unsettled interval is owned rather than assumed.

## 3. Audience

| Audience | What they need from this specification |
|---|---|
| Risk, security, audit | What is enforced, what is evidenced, what is not claimed |
| Platform and engineering | Where the decision point sits and what it must be given |
| Implementers | Numbered obligations, conformance levels, fixtures |
| Counterparties | How to verify evidence without trusting the producer |
| Procurement | The difference between this category and adjacent ones |

## 4. In scope

1. The conceptual model for a governed consequential action.
2. The decision points and the order in which they evaluate.
3. Requirements on the decision, the refusal, the promotion to human approval, and
   the recording of each.
4. Constraints over composition across steps, runs and sessions.
5. The evidence model, including the distinct guarantees of chain, signature and
   external anchor.
6. The unsettled interval after dispatch: the effect states, the fences that hold
   while an outcome is unknown, and who may settle it.
7. Conformance levels and the evidence required to claim one.
8. Explicit non-claims.

## 5. Out of scope

1. How an agent reasons, plans or generates content. This specification constrains
   execution, not cognition.
2. Domain truth. Whether a beneficiary is eligible, whether a payment is lawful, or
   whether a change is safe is supplied by the operator's checks and systems.
3. Sandboxing, isolation or containment of arbitrary processes.
4. Model evaluation, prompt security, or training-time concerns.
5. Settlement, transactionality, or rollback guarantees in downstream systems.
6. Organizational governance: committees, policies, risk appetite, training.
7. Multi-tenant hosting, pricing, or commercial terms.

## 6. Relationship to adjacent categories

| Adjacent category | What it answers | Why it is not this |
|---|---|---|
| Policy engine | may this request proceed? | evaluates a request, not an operation across time and composition |
| API gateway / proxy | is this traffic permitted? | sees transport, not intent or consequence |
| Approval workflow | did a human agree? | ends at the yes; the gap starts there |
| Observability / SIEM | what happened? | reports after the fact; cannot refuse |
| Sandbox / isolation | how much damage is contained? | contains effects; does not decide whether they may occur |
| Agent framework | how does the agent loop? | the loop is the caller, not the governor |
| AI governance platform | is the programme adequate? | organizational assurance, not execution-time enforcement |
| Execution governance (as published by other authors) | three overlapping judgments, or a discipline for deciding before an action becomes an effect | adjacent, not identical: this specification is a set of obligations on a system that **executes**, and it owns the interval after dispatch as well as the decision before it (levels 5–6) |

## 7. Normative language and requirement identifiers

- MUST / MUST NOT / SHOULD / SHOULD NOT / MAY per RFC 2119 and RFC 8174.
- Requirements are numbered `RS-NNN` and grouped by area.
- A requirement may be **conditional** on a declared capability or deployment mode.
  Conditions are stated in the requirement.
- A requirement is **withdrawn** by marking it withdrawn; numbers are never reused.

## 8. Conformance levels

Six levels, cumulative. A deployment declares the highest level it satisfies and
lists the requirements it does not meet. The normative definition, the requirement
lists and the evidence per level are in `09-conformance.md`; this table is a summary
and, where the two disagree, `09-conformance.md` wins.

| Level | Name | What it adds |
|---|---|---|
| 1 | Core effect-gated execution | plan artifact, gate, approval binding, refusal, decision record |
| 2 | Composition | sequence constraints and effect identity across steps and runs |
| 3 | Commit-time authority | dispatch-time revalidation against operator authority |
| 4 | Verifiable evidence | chain, signature, anchor, independent verification |
| 5 | Distributed capacity | aggregate reservation across sessions and replicas |
| 6 | Commitment | ownership of the unsettled interval: write-ahead record, non-terminal `Ambiguous`, fenced dependents, refused retries, evidenced resolution. A Level 6 claim MUST name a durability axis (L1 in-process or L2 contested store) and a settlement axis (none, or L3 provider receipt); the two are independent |

A deployment MUST NOT claim a level whose underlying mode is absent. See
`09-conformance.md` and `10-non-claims.md`.

## 9. Versioning

This specification uses semantic versioning with a specification-specific meaning:

- **Major** — a requirement is added, removed, or made stricter; identifiers may be
  withdrawn.
- **Minor** — a requirement is clarified without changing obligations; new
  informative material; new conformance fixtures.
- **Patch** — editorial only.

Terminology changes that alter an obligation are major, even if the word change
looks small.

## 10. What success looks like

A reader can answer four questions without asking the vendor:

1. Where is the decision made, and what is it given?
2. What happens when it says no?
3. What does the evidence prove, and what does it not?
4. Which level does this deployment satisfy, and what is missing?
