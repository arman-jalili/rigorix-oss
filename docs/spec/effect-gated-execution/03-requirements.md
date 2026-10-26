# 03 — Requirements

**Status: normative.** These are the obligations. Each has a stable identifier, a
rationale, and a trace to the reference implementation or an external anchor.

Normative keywords are capitalised. A requirement marked *conditional* applies only
when the corresponding capability or deployment mode is declared.

---

## A. Plan and scope

**RS-001** A consequential action MUST be expressed as a plan before any step
executes. Free-form execution of consequential steps is not conforming.
*Rationale:* there is nothing to review or enforce without an artifact.
*Trace:* plan/DAG engine; ADR-002, ADR-005.

**RS-002** A plan MUST be acyclic and MUST declare, per step, the tool, the
resolved parameters, the risk class and any declared requirements.
*Rationale:* parameters and order are the substance of what a human approves.
*Trace:* dag-engine; template-system.

**RS-003** The decision to permit a step MUST NOT depend on model output produced
after the plan was sealed.
*Rationale:* an enforcement decision that a model can influence is not deterministic.
*Trace:* no LLM in the dispatch path; ADR-007.

**RS-004** A plan MUST be rejected if it cannot be evaluated deterministically
(malformed rule, unresolvable parameter reference, unknown vocabulary).
*Rationale:* ambiguity must not become permission.
*Trace:* fail-closed evaluation in sequence/precondition modules.

**RS-005** Out-of-scope actions MUST NOT be silently dropped. A step excluded by
policy MUST be recorded as excluded, with the rule.
*Rationale:* silent exclusion hides a decision from the audit.
*Trace:* decision records.

## B. Approval

**RS-010** A step declared as requiring approval MUST NOT dispatch until an
approval for its intent is recorded.
*Rationale:* the declaration must mean something.
*Trace:* approval module; ADR-011.

**RS-011** An approval MUST bind to the canonical intent (tool, resolved payload,
declared scope), MUST be single-use, and MUST carry a validity window.
*Rationale:* binding prevents substitution; single use prevents replay; the window
bounds staleness.
*Trace:* ADR-011; intent digest.

**RS-012** Immediately before dispatch of an approved step, the implementation MUST
re-derive the intent and MUST halt on mismatch.
*Rationale:* a render/approve/execute mismatch is otherwise invisible.
*Trace:* ADR-011; `HaltIntent`.

**RS-013** The identity of the approver MUST be an attested identity, not a
caller-supplied string, and MUST be recorded with the approval.
*Rationale:* an approval without attributable identity is not evidence.
*Trace:* ADR-012.

**RS-014** If approval binding is requested by configuration but required key
material is absent, the implementation MUST fail closed. It MUST NOT fall back to
an unbound approval path.
*Rationale:* a silent downgrade is worse than a startup failure, because it is
invisible.
*Trace:* #1070.

## C. Commit-time authority

**RS-020** Before dispatching a step that matches a configured precondition, the
implementation MUST evaluate the operator's check against current authority.
*Rationale:* approval describes the past; dispatch creates the future.
*Trace:* ADR-017; `DispatchGate`.

**RS-021** A precondition evaluation that is unreachable, times out, returns a
non-success status, produces unparseable output, or returns an unknown
determination MUST refuse the step.
*Rationale:* every ambiguity is on the side of refusal.
*Trace:* ADR-017 failure taxonomy.

**RS-022** A precondition MUST be re-evaluated per dispatch. Its result MUST NOT be
cached across the gap between approval and dispatch.
*Rationale:* caching reintroduces the exact staleness the gate exists to detect.
*Trace:* ADR-017.

**RS-023** A permitting determination from one precondition MUST NOT short-circuit
other matching preconditions.
*Rationale:* one provider's answer is evidence, not authority.
*Trace:* ADR-017 R4 (non-inheritance).

**RS-024** The check that runs MUST be identified by content digest, and the
authority it consulted MUST be identified by digest, both recorded in the decision.
*Rationale:* a reviewer must be able to tell which check ran against which world.
*Trace:* ADR-017; signed findings.

**RS-025** A precondition MUST NOT be writable by the agent it constrains.
*Rationale:* a control the governed party can edit is not a control.
*Trace:* permission R5; authority-demo out-of-repo check.

**RS-026** If a consequential step declares a required companion step, that
companion MUST have completed successfully in the same run before the step may
dispatch.
*Rationale:* presence in a plan is not a prerequisite; ordering is not outcome.
*Trace:* #1069; companion prerequisite at dispatch.

## D. Composition: sequence and effect

**RS-030** An implementation supporting Level 2 MUST evaluate operator-authored
sequence constraints at plan time, before any step executes.
*Rationale:* the earliest decision is the cheapest and the safest.
*Trace:* ADR-013; plan validation.

**RS-031** Sequence constraints MUST also be evaluated at the run prefix, so that
dynamically composed plans cannot bypass plan-time evaluation.
*Rationale:* a plan can change after sealing in some designs.
*Trace:* ADR-013 R3.

**RS-032** A sequence constraint MAY read evidence from previous runs, and when it
does, the evaluation MUST use signed history and MUST fail closed if that history is
unavailable.
*Rationale:* composition can span sessions; missing history is not innocence.
*Trace:* ADR-013 R7.

**RS-033** A sequence constraint's action MUST be either promote or refuse. A
constraint that only warns is not conforming at Level 2.
*Rationale:* a warning is not a control.
*Trace:* ADR-013.

**RS-034** A rule MAY compare a parameter value against a value carried by an
earlier matched step in the same run.
*Rationale:* the same effect can be expressed with different parameters.
*Trace:* ADR-014.

**RS-035** Effect identity MUST be computed from domain-supplied canonical keys. The
implementation MUST NOT infer domain meaning from free text.
*Rationale:* inference is non-deterministic and unauditable.
*Trace:* ADR-014.

## E. Capacity

**RS-040** An implementation supporting Level 5 MUST reserve capacity before
authorizing a step, and MUST resolve the reservation afterwards by commit or
release.
*Rationale:* read-then-increment permits concurrent authorization into the same room.
*Trace:* ADR-018.

**RS-041** A reservation MUST carry a lease and MUST be released if the execution
crashes or is cancelled.
*Rationale:* orphaned reservations silently reduce capacity forever.
*Trace:* ADR-018; release-on-drop guard.

**RS-042** When capacity cannot be determined (ledger unreachable, ambiguous
response), the step MUST be refused.
*Rationale:* unknown capacity is not unlimited capacity.
*Trace:* ADR-018 I4.

**RS-043** A deployment using an in-process ledger MUST NOT claim cross-session or
cross-replica capacity atomicity.
*Rationale:* the guarantee differs by store; claiming otherwise is a false claim.
*Trace:* ADR-018 claim ceiling.

## F. One gate, fail-closed configuration

**RS-050** Every execution route that can cause a side effect MUST pass the same
gate sequence in the same order. A lower-level route that bypasses gates MUST NOT
be exposed or MUST be explicitly marked as ungoverned.
*Rationale:* a second route with different semantics is a hole with a signature.
*Trace:* #1074; `gate.rs`.

**RS-051** A gate that cannot be evaluated MUST refuse. There is no "permit on
error".
*Rationale:* fail-open is a design decision to be exploited.
*Trace:* all gate modules.

**RS-052** Capability controls that are not enabled MUST be reported as absent in a
machine-readable posture, so a deployment cannot believe it is protected by a
control it never configured.
*Rationale:* the most dangerous state is a believed control.
*Trace:* configuration surface (gap to close; see 11-traceability).

**RS-053** Command execution MUST reject compound shell expressions when the policy
is expressed as an allowlist of commands.
*Rationale:* a prefix match is not a command match.
*Trace:* #1067.

**RS-054** A process spawned to execute a step MUST NOT inherit control-plane
secrets (signing keys, anchor credentials).
*Rationale:* the governed command must not be able to forge the record of its own
governance.
*Trace:* #1068.

## G. Decision records and evidence

**RS-060** Every decision (permit, promote, refuse, halt) MUST produce a record
containing the rule identity, the outcome, the step, the time, and the digests of
the inputs consulted.
*Rationale:* a decision without its inputs cannot be reviewed.
*Trace:* ADR-016; envelope findings.

**RS-061** Records MUST be integrity-protected. An unsigned or unanchored record
MUST be marked as degraded rather than presented as intact.
*Rationale:* the absence of a signature is itself a fact.
*Trace:* ADR-016; `evidence_degraded`.

**RS-062** Records MUST support chaining such that removal, reordering or
modification is detectable against a retained history.
*Rationale:* a set of records is not a history.
*Trace:* ADR-016 `prev_hash`, `sequence`.

**RS-063** An implementation supporting Level 4 MUST support an external anchor
whose signature can be verified offline with a public key.
*Rationale:* internal consistency can be produced by anyone, including an attacker.
*Trace:* ADR-016; Ed25519 anchor; SDK `verify_ed25519`.

**RS-064** Verification tooling MUST distinguish, and state, what each verification
establishes: internal consistency, integrity against a key, or external provenance.
*Rationale:* conflating these is the most common evidence overclaim.
*Trace:* `05-evidence-model.md`; SDK verifier surface.

**RS-065** Verification MUST be possible without calling the producer's service.
*Rationale:* evidence that requires trusting the producer is testimony.
*Trace:* offline verifiers in five languages.

**RS-066** Evidence MUST be sufficient to reconstruct why a step was refused,
without access to the agent's conversation.
*Rationale:* the agent's narrative is not evidence.
*Trace:* signed refusal findings.

## H. Interfaces and control-plane integrity

**RS-070** Every interface that can trigger execution (CLI, MCP, HTTP, CI adapter)
MUST route through the same governed path.
*Rationale:* interfaces multiply; guarantees must not.
*Trace:* #1074; server/MCP/actions adapters.

**RS-071** An interface MUST NOT expose a direct execute operation that skips the
gate sequence.
*Rationale:* convenience APIs become the path of least resistance.
*Trace:* SDK governance invariant; catalog subset rule.

**RS-072** Control-plane configuration (rules, preconditions, approvals) MUST NOT be
writable through the execution interface used by the agent.
*Rationale:* separation of duties is structural, not procedural.
*Trace:* permission R5.

**RS-073** The implementation MUST reject a request whose declared scope is broader
than the caller's permission, before planning.
*Rationale:* scope expansion at the boundary is a decision, not a detail.
*Trace:* permission enforcer.

## I. Claims

**RS-080** A conformance claim MUST state the level, the deployment mode, and the
requirements not met.
*Rationale:* an unqualified claim is unfalsifiable.
*Trace:* `09-conformance.md`.

**RS-081** A claim MUST NOT assert a guarantee whose underlying capability is absent
in the declared mode.
*Rationale:* the most damaging failure is a true architecture with a false claim.
*Trace:* `10-non-claims.md`; ADR-018 claim ceiling.

**RS-082** Every public claim about behaviour SHOULD trace to a requirement in this
document, a test, or a runnable artifact.
*Rationale:* traceability is what separates a specification from marketing.
*Trace:* `11-traceability.md`.


## J. Commitment (the unsettled interval)

**RS-090** Before dispatching an effect that is covered by a commitment record, the
implementation MUST durably record the intent. The write-ahead MUST complete before
the effect is sent.
*Rationale:* a record written after dispatch cannot prove anything about the window.
*Trace:* ADR-019 `begin`; write-ahead requirement.

**RS-091** The effect state machine MUST be exactly `Dispatched → Committed |
NotCommitted | Ambiguous`, with `Ambiguous` non-terminal.
*Rationale:* a two-state world forces an unknown outcome to be recorded as a lie.
*Trace:* ADR-019; `EffectState`.

**RS-092** Dependents of an `Ambiguous` effect MUST NOT be released (F1).
*Rationale:* a dependent of an unknown effect is an unknowable consequence.
*Trace:* ADR-019 F1; dependents fenced.

**RS-093** A retry of an `Ambiguous` effect MUST be refused, and the run MUST remain
open (F2).
*Rationale:* retrying can double-pay; silently dropping can lose the work.
*Trace:* ADR-019 F2.

**RS-094** The idempotency key MUST be derived from the intent (dag, node, intent
digest), not from the attempt, and MUST be stable across retries.
*Rationale:* an attempt-derived key makes F2 unenforceable.
*Trace:* ADR-019; `IntentKeyDerivation`.

**RS-095** The implementation SHOULD expose the idempotency key to the provider
command so the downstream system can dedupe (e.g.
`RIGORIX_EFFECT_IDEMPOTENCY_KEY`).
*Rationale:* the engine can refuse a retry; only the provider can refuse a duplicate.
*Trace:* ADR-019; `EFFECT_IDEMPOTENCY_ENV`.

**RS-096** Only a resolver presenting provider evidence MAY leave `Ambiguous`, and
the resolving identity and evidence digest MUST be recorded (F3, F4).
*Rationale:* the party that lost the response cannot know; an unevidenced resolution
is a guess.
*Trace:* ADR-019 F3/F4; `CommitmentError` closed vocabulary.

**RS-097** A deployment claiming cross-process durability MUST use a contested store
and MUST NOT claim it from an in-process record.
*Rationale:* the same claim-ceiling discipline as capacity.
*Trace:* ADR-019 durability axis; enterprise ADR-011.

**RS-098** A deployment MUST NOT claim that an effect committed unless its resolver
has provider evidence (L3). Otherwise it MAY claim only *not-settled*.
*Rationale:* the strongest and most abusable claim in the model.
*Trace:* ADR-019 settlement axis; doctor's derived ceiling.
