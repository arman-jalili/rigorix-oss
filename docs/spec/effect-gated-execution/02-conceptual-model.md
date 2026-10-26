# 02 — Conceptual model

**Status: normative.** This document defines the entities and the decision path.
Requirements that constrain behaviour are in `03-requirements.md`.

---

## 1. The three moments

Every consequential action passes through three moments. Classical authorization
collapses them into one; effect-gated execution keeps them separate and decides at
two of them.

```
  intent
    │
    ▼
┌─────────────────┐   plan artifact: steps, parameters, order,
│   PLAN TIME     │   risk, declared requirements, sequence rules,
│                 │   operator requirements
└────────┬────────┘
         │  promote (requires_approval)
         ▼
┌─────────────────┐   approver identity attested; approval bound to
│    APPROVAL     │   (tool, intent, declared scope); single-use;
│                 │   validity window
└────────┬────────┘
         │
         ▼
┌─────────────────┐   ONE choke point, fixed gate order:
│    DISPATCH     │   1 approval binding re-derived
│  (consequence)  │   2 precondition against live authority
│                 │   3 sequence + companion prerequisite
│                 │   4 capacity reservation
└────────┬────────┘
         │
   ┌─────┴─────┐
   ▼           ▼
permit       refuse  ──► decision record (signed, chained, anchorable)
```

A permit is not a promise about the future. A refusal is a result.

## 2. Entities

| Entity | Identity | Lifetime | Owns |
|---|---|---|---|
| Intent | digest of tool + payload + declared scope | until the action completes | what the human approved |
| Plan | run-scoped id | one run | steps, order, requirements |
| Step | plan-scoped id | one dispatch | one tool invocation |
| Run | run id | start → terminal state | events, decisions, envelope chain |
| Session | caller-supplied id | caller-defined | continuity across runs |
| Approval | digest + approver + window | single use | authority to attempt one intent |
| Precondition | operator-authored id | configuration lifetime | a determination about the world |
| Reservation | reservation id + lease | reserve → commit/release | capacity held |
| Decision | run + step + gate | immutable once recorded | the outcome and its inputs |
| Envelope | producer + sequence | append-only | signed record of an event |

## 3. Relationships

```
  Session 1 ──< Run 1 ──< Plan ──< Step ──< Decision ──< Envelope
      │                       │        │
      │                       │        └──< Refusal (a Decision kind)
      │                       └──< Approval (binds to Intent)
      │
      └──< Run 2 ──< Plan ──< Step ──< Decision   ← cross-run constraint reads
                                                      Run 1's envelopes
  Capacity: reservations exist across runs and sessions, held by a ledger
```

Two things to notice. Composition is a property of the graph, not of a single node.
And evidence outlives every entity above it: the envelope is the only artifact
designed to be read by someone who was not there.

## 4. Gate order

The order is fixed and normative. It exists because each gate changes what the next
one should evaluate.

1. **Approval binding.** Does the intent about to dispatch match the intent that was
   approved? Failure halts; the step does not dispatch and re-approval is required.
2. **Precondition.** Does the operator's check still say yes, against current
   authority? Failure refuses the step.
3. **Sequence and companion prerequisite.** Is this step permitted given the run
   prefix and prior runs? Has every required companion step completed successfully
   before it? Failure refuses the step.
4. **Capacity.** Does this step fit the aggregate budget? Failure refuses the step.

Rationale for the order: identity of the action is checked before its admissibility
(a different action should not consume a capacity reservation), and authority is
checked before ordering (an unauthorized step should not be able to satisfy a
companion prerequisite).

## 5. Decision outcomes

| Outcome | Meaning | Effect on the run | Recorded as |
|---|---|---|---|
| Permit | the step may dispatch | continues | decision, permit |
| Promote | a human must approve first | run pauses | decision, approval request |
| Refuse | the step must not dispatch | step fails; dependents skipped | decision, refusal finding |
| Halt | the approved intent no longer matches | run halts, re-approval required | decision, intent mismatch |

`Refuse` and `Halt` are different. A refusal is a rule doing its job. A halt means
the execution is no longer the thing that was approved.

## 6. Step-outcome propagation

A failed or refused step MUST NOT release its dependents. Dependents become
`Skipped`. This is what makes "the payment only runs if the authority check ran and
succeeded" expressible: the obligation is a dependency on outcome, not on presence.

## 7. State model

Run: `Pending → Running → (Completed | Failed | Cancelled)`; `Running → Paused`
while awaiting approval.

Step: `Pending → InProgress → (Completed | Failed | Skipped)`; a promoted step is
`Pending` with an outstanding approval request.

Dependents of a `Failed`, `Skipped` or `Refused` step never enter `InProgress`.

## 8. Where authority lives

Three separable things are often conflated:

- **Identity** — who is acting, attested.
- **Permission** — what the caller is allowed to attempt, as configured.
- **Authority** — whether the attempt is admissible in the world *now*.

Identity is established once. Permission is configured. Authority is re-read at
dispatch, from a source the operator controls. A system that checks only the first
two has an approval, not a decision.

## 9. Composition, precisely

Composition has four axes, and each is a separate question:

| Axis | Question | Fails when |
|---|---|---|
| Sequence | may this happen given what already happened | order or prerequisite violated |
| Authority | is the yes still true | world changed after approval |
| Effect | is this the same consequence | identity resolved differently |
| Capacity | what do these sum to | aggregate exceeds budget |

An implementation can satisfy one axis and not the others. Conformance levels make
that explicit, so a buyer knows which questions are answered.

## 10. What the model deliberately leaves outside

- Domain truth (eligibility, lawfulness, safety of a change).
- Transactional guarantees in downstream systems.
- Process isolation.
- The agent's reasoning.

These are inputs, or separate layers. Including them would make the model
unfalsifiable.


## 11. The interval after the act

The gates in §4 all run before a step dispatches. They cannot answer what happened
after it left.

```
Intent ──begin──► Dispatched ──settle(committed)─────► Committed
                      │        └─settle(not_committed)─► NotCommitted
                      ├──mark_ambiguous──► Ambiguous
                      │                       │
                      └──deadline/sweep───────┘
                                              └──resolve(provider evidence)──► Committed | NotCommitted
```

`Dispatched` is the write-ahead state: the record is durable before the effect is
sent. `Ambiguous` is non-terminal by design. Leaving it requires a resolver and
provider evidence; a lapsed lease is absence of evidence, not evidence of absence.

Fencing rules (normative in EGE-090…EGE-098): dependents of an ambiguous effect are
not released (F1); a retry is refused (F2); only provider evidence settles (F3); the
dispatching process and the agent are never the resolver (F4).

## 12. Two ceilings

An implementation's commitment capability is a cell, not a level:

|  | In-process record (L1) | Contested store (L2) |
|---|---|---|
| No receipt reader | durable within the process; proves not-settled | survives restart, shared; proves not-settled |
| Provider receipt (L3) | can settle; loses the record on restart | can settle and the record survives |

Durability comes from the backend. Settlement comes from the receipt. A deployment
MUST state both.
