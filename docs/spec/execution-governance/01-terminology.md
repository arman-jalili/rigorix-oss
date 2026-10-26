# 01 — Terminology

**Status: normative for this specification.** These definitions constrain the other
documents in this set — requirements, evidence, conformance — and nothing outside
them. Where another author defines one of these terms differently in their own
framework, their definition governs theirs and this one governs this document; a
shared word is not a shared claim.

Each entry gives the definition, the words to avoid, and where the concept is
implemented. Terms are grouped by the part of the model they belong to.

---

## A. The category and the unit

### Execution governance
The discipline of deciding again at the moment an action becomes an effect, and
proving what was decided. It comprises five commitments: a bounded plan, authority
revalidated at dispatch, constraints on composition, ownership of the interval
between dispatch and settlement, and independently verifiable evidence.

Used here descriptively. It is a category name shared with other authors (see
`README.md` for the prior work), not a term this document owns.

*Avoid:* runtime governance (a generic category claim), AI governance (an
organizational programme), agent security (inputs and prompts), guardrails
(perimeter controls).

*Anchor:* this specification.

### Effect-gated
The adjective for the mechanism, not the category: a gate placed at the effect — the
moment an action becomes real — rather than at plan time or at approval. "An
effect-gated step", "the effect-gated path". Not a noun, and not a synonym for the
discipline.

*Avoid:* gatekeeper, guardrail (both describe something the model is asked to obey).

### Consequential action
An action whose effects are real and not trivially reversible: money moves, access
is granted, data changes, infrastructure shifts, a regulated record is written. The
unit of analysis. Actions that cannot cause harm are out of scope.

*Avoid:* tool call (an implementation detail), request (too narrow: a consequence
may span several requests).

### Effect
The real-world change produced by an action, independent of how it was expressed.
Two actions with different identifiers can share one effect; one action can have
several effects.

*Anchor:* ADR-014.

### Effect key
A canonical, domain-supplied identifier for an effect. The engine compares effect
keys; it does not infer what they mean.

*Avoid:* beneficiary id (one possible source of an effect key, not the concept).

### Composition
The property that emerges when individually permitted actions combine, across
steps, runs or sessions, into an outcome no single decision was permitted to reach.
Composition is an architectural property, not a rule inside an isolated evaluator.

---

## B. The plan and its parts

### Intent
What the caller asks for, expressed as an action with parameters and a declared
scope. Intent is what a human approves. It is not a node identifier and not a
transcript.

### Plan
The reviewable artifact produced from intent: an ordered, acyclic set of steps with
parameters, dependencies, risk classes and declared requirements. Planning may
involve a model; the plan, not the model, is what is enforced.

*Avoid:* workflow (implies an authored, static process), prompt (not an artifact).

### Step
One unit of execution inside a plan, bound to one tool invocation and its
parameters. A step is the granularity at which gates evaluate and refusals are
recorded.

### Run
One execution of one plan, from start to terminal state. A run has an identity, a
state, an event stream and a decision record.

### Session
A caller-side continuity (a conversation, an agent loop, a work period) that may
contain several runs. Sessions matter because composition can span them.

### Plan time
The moment a plan is constructed and evaluated before any side effect. Constraints
that can be decided statically are decided here.

### Dispatch
The moment immediately before a step's tool is invoked and the effect becomes real.
The moment of consequence.

---

## C. The decision

### Gate
A deterministic evaluation that can permit, promote or refuse a step. Gates run in
a fixed order at a single choke point.

*Avoid:* check (too weak: a check may warn; a gate decides), policy (a gate may
implement policy, but a gate is the decision point).

### Decision
The outcome of a gate: permit, promote (route to human approval), or refuse. Every
decision is recorded.

### Refusal
A decision that a step MUST NOT dispatch. A refusal is a successful control, not an
error. It is recorded with the rule that produced it and the evidence it saw.

*Avoid:* failure (a refusal is an intended outcome), block (implies a different
mechanism).

### Promotion
A decision that a step requires human approval before dispatch. Promotion is the
default posture for consequential steps: the system does not ask, it either refuses
or requires a human.

### Approval
A human decision to permit a specific intent. Recorded with the approver's attested
identity, the intent it applies to, and a validity window.

*Avoid:* sign-off (a process term), confirmation (implies a UI action).

### Approval binding
The property that an approval is cryptographically tied to the intent it approved,
single-use, and time-bound, and is re-derived at dispatch to detect substitution or
replay.

*Anchor:* ADR-011.

### Authorization decay
The condition in which a valid approval describes a world that no longer exists.
The approval is unchanged; the world moved. The failure this layer exists to
prevent.

### Commit-time authority
Authority revalidated at the moment of consequence, against the operator's current
authority source, rather than at plan or approval time.

*Avoid:* re-check (informal), revalidation (acceptable synonym in prose, not a term).

### Precondition
An operator-authored, deterministic check evaluated immediately before a step
dispatches. It returns a determination; it does not negotiate. Failure, absence,
timeout, unparseable output and unknown all refuse.

*Anchor:* ADR-017.

### Authority source
The operator-controlled system that holds the truth a precondition consults: an
entitlement store, a sanctions list, a freeze table, a change calendar. The engine
does not hold it and does not interpret it.

### Operator
The party that owns the rules, the authority sources and the deployment. The
operator writes the checks; the agent does not.

---

## D. Composition

### Sequence constraint
A rule over ordered steps that either promotes or refuses a matching step. Evaluated
at plan time, at the run prefix, and across prior runs using signed history.

*Anchor:* ADR-013.

### Cross-run constraint
A sequence constraint whose evaluation reads evidence from previous runs, so that an
attack split across sessions still matches.

### Capacity
A finite resource that several independent executions compete for: spend, quota,
concurrency allowance, rate allowance.

### Reservation
An atomic claim on capacity taken before authorization and resolved afterwards by
commit or release. The unit that prevents two executions from authorising into the
same room.

*Anchor:* ADR-018.

### Exposure
The aggregate amount of capacity currently committed plus reserved. Exposure is the
quantity a capacity rule constrains.

---

## E. Evidence

### Decision record
The record of one decision: the rule, the inputs it saw, the outcome, the time, and
the digests that allow a reviewer to check what was consulted.

### Envelope
The signed, structured record of an execution or step event. Carries identity,
timing, outcome, findings and integrity metadata.

*Anchor:* ADR-016, SDK envelope schema (v2).

### Chain
The linkage of envelopes by `prev_hash` and `sequence`, so that removal, reordering
or modification is detectable against a retained history.

### Anchor
An external, signed statement about a history: a checkpoint or slice signed with a
key the producer does not control at verification time. An anchor converts internal
consistency into externally checkable provenance.

*Avoid:* timestamp (an anchor may include one; it is not one).

### Receipt
A signed summary binding a specific history state to a specific identity and time,
verifiable offline.

### Verifier
A program that checks evidence without trusting the producer. Verification is a
computation over artifacts and public keys, not an act of faith.

### Verification level
A statement of what a given verification establishes. Defined in
`05-evidence-model.md`; levels are not interchangeable.

### Degraded evidence
Evidence that is internally consistent but lacks a required integrity property
(no signature, no anchor). Degradation MUST be marked in the record, never implied.

---

## F. Actors

### Agent
The actor that proposes actions and generates content. The agent is untrusted with
respect to the rules. It may not author or modify the controls that govern it.

### Approver
A human with authority to approve a specific intent. Identity is attested, not
asserted.

### Auditor
A party that examines evidence after the fact, often without access to the
producer's systems. The auditor is the reason evidence must be verifiable offline.

### Implementer
A party building a conforming system. Implementers consume `03-requirements.md` and
`09-conformance.md`.

---

## G. Commitment

### Effect commitment
The discipline of owning the unsettled interval between dispatching an effect and
learning whether it committed. Answers one question: *was this effect committed?*
Authority for the answer is external: the provider's receipt or read-back.

*Anchor:* ADR-019; `effect-commitment` context.

### Unsettled interval
The window after dispatch in which the outcome is unknown: the response was lost,
the process died, or a deadline passed. Collapsing it into "completed" or "failed"
is a lie with a timestamp.

### Ambiguous
A non-terminal effect state: the effect may or may not have happened. It leaves only
through a resolver presenting provider evidence. Dependents are fenced; retries are
refused; the run stays open.

### Resolver
The party permitted to settle an `Ambiguous` effect. The dispatching process and the
agent are never the resolver: they are the parties that provably cannot know.

### Provider receipt
Evidence from the downstream system that an effect committed (receipt id, read-back
digest, observed time). The only authority that can establish settlement.

### Fence
A rule that prevents a dependent or a retry from proceeding while an effect is
unsettled. F1 dependents; F2 retry; F3 resolver evidence; F4 resolver identity.

### Durability
One of two independent ceilings. Can the record survive process death and be shared
across processes? L1 in-process; L2 contested store. Says nothing about whether the
effect happened.

### Settlement
The other ceiling. Can you prove the effect happened? Requires provider evidence.
Without it, a deployment can prove *not-settled* and never *settled*.

> Durability and settlement are orthogonal. A claim that names one without the other
> is incomplete, the same way a chain claim that omits the anchor is incomplete.

## H. Words we do not use as our own

| Avoid | Why | Use |
|---|---|---|
| runtime governance | claimed by other frameworks; generic | execution governance |
| AI governance | organizational programme, compliance | execution governance |
| agent security | prompts, inputs, tool poisoning | execution governance (for the execution layer) |
| guardrail | perimeter metaphor, no decision semantics | gate |
| human-in-the-loop | describes a UI pattern, not a control | approval, then commit-time authority |
| policy enforcement point | one check at one boundary | gate, commit-time authority |
| tamper-proof | impossible | tamper-evident |
| immutable ledger | overclaim; append-only and verifiable are the real properties | chained, anchored history |

The last two matter. A record can be tamper-evident without being immutable, and
claiming immutability invites a demonstration that it is false.
