# Execution Governance

*A working definition. The normative version is in [`spec/`](https://rigorix.eu/spec/).*

---

## 1. The class of problem

Authorization is usually decided once, before an action executes, and then treated as
a property of that action. It works when the decision and the effect happen in the
same moment.

Automated systems separate them. A plan is made, a human approves, the execution
resumes later, in parallel, across sessions and agents. The decision stays fixed
while the world moves.

Four scenes, four domains, one shape.

**A payment.** Approved on Monday. On Tuesday the beneficiary is frozen. The approval
is unexpired and correctly signed. The payment must not run.

**An access grant.** A manager approves a contractor for one project. The grant lands
in a group that also carries production access. The approved intent and the effective
permission are no longer the same thing.

**A schema migration.** The change is approved during a window. Before it runs, a
freeze is declared for the quarter close. The approval did not change. The permission
to execute did.

**A delegated task.** An orchestrator agent hands a narrowed capability to a
sub-agent. The sub-agent delegates again. Each hop is individually valid. The chain
grants more than the original authority ever allowed.

The failures are not model failures. They are decisions made once and effects
arriving later, or several permitted actions composing into an outcome nobody
permitted.

---

## 2. Definition

**Execution governance is the discipline of deciding again at the moment an
action becomes an effect, and proving what was decided.**

Five commitments follow.

1. A consequential action runs inside a bounded plan, reviewable before it executes.
2. Authorization attaches to an intent and is revalidated against the world at
   dispatch.
3. Constraints apply to composition across steps, runs and sessions.
4. Every decision produces evidence a third party can verify without trusting the
   producer.

A system that does one of these is a control. A system that does all four is a
governance layer.

---

## 3. Three moments

Every consequential action passes through plan time, approval, and dispatch.

Most systems collapse them into one decision, made at the first moment and applied at
the third. The collapse is the bug. The moments are separated in time, and time is
where authority decays.

The layer decides at plan time and again at dispatch. Approval records a human's
judgment about one specific intent, bound so it cannot be replayed against another.

---

## 4. Four axes

An action can be individually permitted and collectively wrong in four ways.

**Sequence.** May this happen, given what already happened? Ordered rules, evaluated
before the plan is sealed, at the run prefix, and across prior runs using their signed
history.

**Authority.** Is the yes still true? An operator-authored check runs immediately
before the step's tool is invoked, reads live authority, and refuses on anything other
than a definite permit.

**Effect.** Is this the same consequence under a different name? Canonical effect keys
supplied by the domain, compared deterministically.

**Capacity.** What do these actions sum to? Capacity is reserved before authorization
and resolved after execution, so concurrent sessions cannot authorize into the same
room.

Each axis is a separate question with a separate enforcement point. An implementation
can answer one and not the others, which is why conformance is levelled.

---

## 5. Evidence

A refusal is a result, not an error. It is recorded with the rule that fired, the exit
code, the digest of the input, the digest of the check that ran, and the digest of the
authority it consulted.

Three integrity properties are often conflated, and they are not the same:

- a **chain** proves internal consistency within the set you were given;
- a **signature** proves integrity against a key;
- an **anchor** signed by a party that does not control the producer proves external
  provenance.

Only the third survives a compromised producer. A verifier must say which one it
achieved. This is specified in [`spec/05-evidence-model.md`](https://rigorix.eu/spec/05-evidence-model/).

---

## 6. One gate

Every route that can cause a side effect passes the same gate sequence, in one place:
approval binding, precondition, sequence and companion prerequisites, capacity
reservation.

That property is recent and it matters. A graph route and a single-node route used to
answer differently for the same tool. The gate now lives in one module and both call
it. The same question produced four adjacent fixes: a compound shell string that
passed a prefix allowlist, a subprocess that inherited the signing key, a companion
step satisfied by presence rather than success, and an approval binding that silently
downgraded when its key was missing.

Each has a test that fails against the old behaviour. That is the standard: not "the
architecture says so", but "here is the input that used to get through".

---

## 7. What this is not

A sandbox contains effects. A policy engine answers about a request. An approval
workflow ends at the yes. Observability reports after the fact. An agent framework
owns the loop.

This layer governs the execution the loop produces. The full comparison, including
where each adjacent category stops, is in [`spec/00-scope.md`](https://rigorix.eu/spec/00-scope/) §6.

---

## 8. What we refuse to claim

A control whose limits are stated can be used by a risk team. One that overstates
itself cannot, because the first question is what it does not cover.

The layer does not close the window between check and effect; downstream systems
still enforce at their own commit point. It does not know domain truth. It is not a
sandbox. It does not make downstream operations transactional. In the current
reference implementation, effect-scope evidence is opt-in, cross-session capacity
requires the control plane, control-posture reporting is incomplete, and two
enterprise ingest defects are open.

The full list is normative: [`spec/10-non-claims.md`](https://rigorix.eu/spec/10-non-claims/).

---

## 9. Why this layer has to exist

Banks have run maker-checker for generations. Two signatures, and a re-check at
release, because the world moves between the approval and the payment. Operations
teams have change freezes and pre-flight calls. Militaries re-verify at the point of
action rather than at the point of planning.

The practice is old. The automation is new. What did not exist was a deterministic
layer that performs the second check at machine speed, for agents, and produces
evidence a third party can verify afterwards.

This does not invent a control. It industrializes one that regulated industries
already trust.

---

## 10. Where to go next

| If you want | Read |
|---|---|
| The definitions, and the words we refuse to use | [`spec/01-terminology.md`](https://rigorix.eu/spec/01-terminology/) |
| The model: entities, gate order, states | [`spec/02-conceptual-model.md`](https://rigorix.eu/spec/02-conceptual-model/) |
| The numbered obligations | [`spec/03-requirements.md`](https://rigorix.eu/spec/03-requirements/) |
| What evidence proves, and what it does not | [`spec/05-evidence-model.md`](https://rigorix.eu/spec/05-evidence-model/) |
| The attack classes and residual risks | [`spec/07-threat-model.md`](https://rigorix.eu/spec/07-threat-model/) |
| How the model applies to seven domains | [`spec/08-applicability.md`](https://rigorix.eu/spec/08-applicability/) |
| Levels, claims and self-test | [`spec/09-conformance.md`](https://rigorix.eu/spec/09-conformance/) |
| The limits | [`spec/10-non-claims.md`](https://rigorix.eu/spec/10-non-claims/) |
| Which requirement is implemented where | [`spec/11-traceability.md`](https://rigorix.eu/spec/11-traceability/) |

Start with [`spec/`](https://rigorix.eu/spec/) if you want the reading paths by role.

---

## 11. The ask

Adopt the terms if they are useful. Test the claims. Tell us where they break.

The interesting argument is not whether governance is good. It is which decisions have
to be made twice, and what has to be recorded the second time. Our answer: every
decision whose consequence arrives after the world it was made in. That is most of the
ones that matter.
