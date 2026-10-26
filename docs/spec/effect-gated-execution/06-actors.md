# 06 — Actors and responsibilities

**Status: informative.**

---

## 1. Why roles matter here

Effect-gated execution only works if the party being governed cannot edit the rules,
approve its own actions, or rewrite the record. That is a separation-of-duties
property, and it is structural: it lives in who can write what.

## 2. Roles

| Role | Owns | May write | Must not |
|---|---|---|---|
| Operator | rules, preconditions, authority sources, deployment | control-plane config, outside the agent workspace | approve its own agent's actions where policy forbids it |
| Agent | proposing actions, generating content | its own workspace | rules, preconditions, approvals, evidence |
| Approver | a decision about one intent | approvals | change the intent, extend its own authority |
| Auditor | examination | nothing | alter evidence |
| Implementer | a conforming system | code | claim a level it does not satisfy |
| Anchor holder | external attestation | anchor signatures | be the same party as the producer, for P3 |

## 3. Responsibility matrix

| Control | Operator | Agent | Approver | Auditor |
|---|---|---|---|---|
| Define sequence rules | R | — | — | C |
| Define preconditions | R | — | — | C |
| Propose a plan | A | R | — | — |
| Approve an intent | — | — | R | — |
| Dispatch decision | — | — | — | — (the gate) |
| Produce evidence | A | — | — | — |
| Verify evidence | — | — | — | R |

R = responsible, A = accountable, C = consulted.

The empty row for "dispatch decision" is deliberate. No human role decides at
dispatch; the gate does, using the operator's rules. Humans decide what the rules
are and what to approve, not what to permit at the moment of consequence. Otherwise
the layer is a queue with extra steps.

## 4. Anti-patterns

**The operator who is also the anchor.** Produces history and attests it. P3
collapses into P2.

**The approver who can edit the plan.** Approval becomes a formality.

**The agent with write access to `.rigorix/`.** It can widen its own permissions.

**The auditor who calls the vendor's API.** Verification becomes testimony.

**The platform team that runs a second execution path "for convenience".** The gate
is bypassed by the path nobody reviews.

## 5. Minimum viable separation

For a single-team deployment, the minimum is: rules outside the agent's write scope,
approvals attributed to a human identity, evidence verifiable offline. That is
achievable without an enterprise plane and is enough for Level 1–3.

For regulated deployment, add: distinct anchor holder, distinct approver population,
and an auditor who verifies without vendor cooperation.
