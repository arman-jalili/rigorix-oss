# 07 — Threat model

**Status: informative.** Each attack class maps to a control and to the residual
risk that remains.

---

## 1. Adversaries

| Adversary | Capability | Goal |
|---|---|---|
| Misaligned or manipulated agent | proposes actions, generates content, may be prompt-injected | cause an effect outside its authority |
| Careless operator | configures rules, approves under time pressure | keep the system working, skip controls |
| Malicious insider | holds a legitimate role | perform an action without attributable evidence |
| Compromised producer | controls the engine host or its keys | present a history that did not happen |
| External attacker | can influence inputs, tools or models | achieve an effect through the agent |
| Sibling session | another authorized execution | consume shared capacity or compose an unauthorized outcome |

The model assumes the agent is untrusted with respect to the rules and assumes the
producer may be compromised. It does not assume the operator's authority sources are
honest; that is a separate trust anchor.

## 2. Attack classes

### T1 — Stale authority
Approve at T₀, change the world at ΔN, dispatch at Tₙ.
**Control:** precondition at dispatch, re-read per step, fail-closed (RS-020–RS-025).
**Residual:** the window between check and effect remains. The downstream system must
enforce at its own commit point.

### T2 — Composition
Individually permitted steps whose order or combination is prohibited.
**Control:** sequence constraints at plan, prefix and cross-run; effect identity
(RS-030–RS-035).
**Residual:** rules express only what the operator thought of. Novel compositions pass.

### T3 — Cross-session splitting
Divide an attack across two prompts or sessions so no single run matches.
**Control:** cross-run evaluation over signed history (RS-032), fail-closed when
history is unavailable.
**Residual:** history must be retained and available; retention policy is a
deployment decision.

### T4 — Approval substitution
Get a human to approve a benign intent, dispatch a different one.
**Control:** intent digest, single-use approval, re-derivation at dispatch (RS-011,
RS-012).
**Residual:** approval fatigue; a human who does not read the payload cannot be
protected by binding.

### T5 — Scope expansion
Start with a narrow permission and widen it through subsequent actions.
**Control:** declared scope in the intent, permission gates per step (RS-073).
**Residual:** a chain of individually in-scope actions that widens effect; this is T2.

### T6 — Aggregate exposure
Many sessions each within a per-session cap, collectively over the budget.
**Control:** reserve-then-authorize against a shared ledger (RS-040–RS-043).
**Residual:** only when the ledger is a contested store. In-process ledger gives
single-execution atomicity and no more.

### T7 — Control-plane tampering
The agent edits rules, preconditions, approvals or evidence.
**Control:** write denial for the governed party, checks outside the agent-writable
workspace, separate control-plane credentials (RS-025, RS-072).
**Residual:** a compromised host with the operator's own credentials is out of scope.

### T8 — Secret inheritance
A dispatched command reads the engine's signing key and forges evidence.
**Control:** cleared child environment (RS-054), secrets outside the execution
workspace.
**Residual:** same-UID filesystem access; process isolation is a deployment concern.

### T9 — Evidence forgery
Construct a self-consistent chain that never happened.
**Control:** V3 external anchor; signature by a key the producer does not hold
(RS-063).
**Residual:** if the anchor key is held by the producer, only P2 is achieved. This is
why the anchor holder must be a separate party.

### T10 — Bypass by alternate route
Use a lower-level API that skips the gate.
**Control:** one gate module for all routes; no direct-execute interface (RS-050,
RS-070, RS-071).
**Residual:** side channels outside the engine (a shell the agent runs itself, a
direct database credential). The engine governs what is routed through it.

### T11 — Silent downgrade
A control is requested but not active, and nothing says so.
**Control:** fail closed on missing configuration (RS-014, RS-051); machine-readable
control posture (RS-052).
**Residual:** posture reporting covers only the seams `rigorix doctor` knows. A new
control seam that is not added to that report is invisible, which is the same failure
with a shorter history (RS-052).

### T12 — Refusal laundering
The agent retries a refused action through a different tool or path until one passes.
**Control:** sequence constraints across steps and runs; capacity and effect keys
(RS-030–RS-035).
**Residual:** a genuinely different tool with the same effect may not match the rule
unless the operator expressed it by effect rather than by tool.

## 3. Out of scope

- Compromised hardware, hypervisor or supply chain.
- An operator who deliberately falsifies authority sources.
- Collusion between approver and agent.
- Model-level attacks that never reach a governed action.
- Denial of service against the engine.

## 4. Using this model

A conformance claim should cite which attack classes are covered at the declared
level. "Level 3" answers T1 and partially T8/T11; it says nothing about T9. A buyer
comparing two systems should compare their residual risk, not their feature lists.
