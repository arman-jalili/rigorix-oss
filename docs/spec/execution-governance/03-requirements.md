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
declared scope), MUST be single-use, and MUST carry a validity window. The approval
evidence MUST state `intent_hash_provenance = derived` — the re-deriving engine
computed the hash from the sealed graph. An `asserted` hash (a caller supplied it and
an attesting service recorded who said it) satisfies RS-013 attribution, **not**
RS-011.
*Rationale:* binding prevents substitution; single use prevents replay; the window
bounds staleness. A hash nobody re-derived is a claim, not a binding.
*Trace:* ADR-011; intent digest; approval ref `intent_hash_provenance`.

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

**RS-067** The signed record MUST NOT contain parameter values, raw check output, or absolute paths.
Parameter presence is recorded, never parameter values; check output is recorded only where the
operator opts in, and is then truncated and redacted. A message that can reach the signed record
MUST name an artifact by basename plus a one-way hash of its path, and MUST be redacted where the
error is **constructed** rather than where it is recorded — a composition root forwards error
strings into a fail-closed gate's detail and from there into every refusal.
*Rationale:* redaction at the point of recording is redaction after the value has already travelled
through the paths that copy strings.
*Trace:* `SpanPrivacy`; the authority-demo's record; `no_absolute_paths_or_username_in_the_canonical_bytes`.

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

**RS-074** A host that is not the engine's own process MUST be able to evaluate the gate with no
network and no engine runtime: the decision MUST be loadable as a module whose operation is a field
inside the request, so that the exported surface cannot grow by accident.
*Rationale:* three exports and one named operation is a surface a reviewer can enumerate; a fifth
export appears without anyone deciding it should exist.
*Trace:* `wasm/`; the six-import table in `wasi/ci/imports.txt`; `verify-published-module.mjs`.

**RS-075** The module's import section MUST be exactly the declared set, and a host MUST refuse a
module whose imports differ, naming what is unaccounted for. A response MUST be length-prefixed and
MUST NOT be trusted by the host without checking the length prefix.
*Rationale:* the trust property is the artifact plus the embedder's import table; a module that
imports a filesystem handle can read one, and a host that trusts a length field can be made to
over-read.
*Trace:* `wasi/ci/imports.txt`; `--self-test` in `check-imports.sh`; the `proc_exit`-raises test.

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

## K. The boundary: what the engine may execute, read and ship

Added in 0.4.0. These requirements state obligations the reference implementation took on
in the batches behind `rigorix-oss` #1478–#1502 and ADR-023. They are not new
capabilities; they are the obligations that made the capabilities honest.

**RS-099** A deployment MUST declare its isolation regime (`container`, `vm`) before a
step that can execute a program may run. An absent, unrecognized or `none` declaration
MUST refuse the step, with a named failure type, **before any process is started**.
*Rationale:* a policy over an argv string is not a boundary. The boundary is what the
deployment states it has; a step that runs before stating it has none.
*Trace:* `SandboxDeclaration`; failure type `sandbox_undeclared`; §L (RS-111).


**RS-100** A claim MUST NOT present the declaration as evidence of containment. What the
implementation may claim is that an undeclared step does not run, never that a declared
step is contained. Inspecting the namespace is not something the engine does.
*Rationale:* a declaration is a fact the operator supplies, not a property the engine
checks; conflating them is the claim `10-non-claims.md` 21 refuses.
*Trace:* `03-requirements.md` §L; ADR-023 §5.

**RS-101** The set of step kinds that can execute a program MUST be derivable from the
implementation's source by a check, and a kind that spawns a process but is not in that
set MUST fail the build.
*Rationale:* a gate keyed on a remembered name is forgotten when the next kind is added;
a gate keyed on a list plus a structural test is not.
*Trace:* `EXECUTING_STEP_KINDS`; `every_spawning_step_kind_is_gated`.

**RS-102** An unrecognized value for a configuration key that selects a permission or
isolation regime MUST be a startup error. It MUST NOT be absorbed by the permissive
default.
*Rationale:* a typo (`read-only` for `read_only`) that silently selects the permissive
regime is a downgrade nobody chose.
*Trace:* `PermissionMode::parse_host_value`; `RIGORIX_PERMISSION_MODE` in all three hosts.

**RS-103** Every boundary crossing the implementation performs — process spawn,
filesystem write, network, environment read — MUST be inventoried with (a) its mitigation
class, (b) what that mitigation depends on, and (c) **what it does not do**. A crossing
that is not in the inventory MUST fail a check.
*Rationale:* the three properties are what turn "we validate the path" into "here is what
the validation does not cover"; the third one is the sentence nobody writes unprompted.
*Trace:* `engine/.pi/architecture/sink-register.tsv`; `validate-sink-register.sh` (one row
per call site, not per file); ADR-023.

**RS-104** A path the implementation validates MUST be mutated through the validated
handle. Validation and mutation MUST NOT be separate resolutions of the same string.
*Rationale:* a check and a use in separate events are not a control; between them the
path means something else.
*Trace:* `tools/infrastructure/fs_at.rs`; the component-swap test; ADR-023 §4.

**RS-105** A decision record MUST carry an evidence-completeness status: `complete`,
`degraded` (with a reason and a count) or `not_requested`. No host MAY present a degraded
run as complete.
*Rationale:* "the run succeeded and the evidence for it did not" is a fact about the run,
not a warning on stderr.
*Trace:* §G (decision records); `EvidenceStatus`; the CLI status bar; the MCP run
response's `evidence` field.

**RS-106** A bound artifact MUST be verified against its recorded digest at load, and a
module whose import section differs from the recorded one MUST be refused by name. A
published digest MUST state the platform and commit it belongs to.
*Rationale:* the import table is cheap to reproduce in a hostile module; the digest is
not. A digest without its build context pins nothing.
*Trace:* ADR-022 §6; `loader.verify_artifact` (Python), `gate.Open`
(Go); the release page's `.wasm` + `.sha256`.

**RS-107** A behaviour the specification names a *read* MUST NOT change repository state
or write a file, and the implementation MUST refuse the subcommands and flags that would.
*Rationale:* a gate that classifies a step by its name is only as good as the name.
*Trace:* `GIT_READ_SUBCOMMANDS`; `validate_git_read_args`; the refusing list for
`symbolic-ref`, `branch -D`, `tag -d`, `--output`.

**RS-108** A refusal kind, an evidence kind, or a gate identifier MUST be added only with
a specification version that names it and a conformance vector that exercises it. A wire
word MUST have exactly one owner in the implementation, and a host MUST be able to read it
from that owner's spelling.
*Rationale:* the three vocabularies are what a verifier parses; an unannounced addition is
a record a published verifier cannot read.
*Trace:* §L (RS-111); `RefusalKind::as_str` (exhaustive).

## L. The five gates

A governed step passes one gate, and the gate asks five questions: four it evaluates in a fixed
order, and one that is answered by a signature bound to the consequence. This section presents
the five — what each decides, where it sits, and what it must not decide. The requirement numbers
are cited rather than restated: one statement, one place.

| Gate | Identifier in the record | The question it asks | Decision | Requirements |
|---|---|---|---|---|
| **Preconditions** | `preconditions` | may this step dispatch at all? | ADR-017 | RS-020–RS-026 (§C), L.1 |
| **Requirements** | `requirements` | which operator obligations does the step leave unmet? | ADR-013 R9, ADR-015 | RS-030–RS-035 (§D), L.2 |
| **Capacity** | `capacity` | does the declared cap admit this consumption? | ADR-018 / ADR-010 | RS-040–RS-043 (§E), L.3 |
| **Effects** | `effects` | has this effect already committed? | ADR-019 | RS-090–RS-098 (§J), L.4 |
| **Approval** | — (a signature, not a gate identifier) | has a human bound to this consequence signed it off? | ADR-011 | RS-010–RS-014 (§B), L.5 |

A sixth decision is added by adding a row here and its requirements in its own section — not by
adding a document, an identifier the record does not carry, or a vocabulary word (§L.6).

### L.1 Preconditions — may this step dispatch at all?

The gate at the choke point, and the only one whose inputs are the operator's own. Its primitive
is small and closed: **match a step → run an operator-authored check → refuse on failure → record
the outcome.** Loops, chained checks, retries that change a verdict, a policy DSL and multi-step
orchestration are not part of it; a use case needing them is a different module (ADR-017
§Non-goals).

**Evaluation order at the choke point.** For the node about to dispatch, the order is fixed:
approval verification (ADR-011) → precondition evaluation (this section) → sequence-policy prefix
(ADR-013 R3) → spawn. A refusal at the precondition step records a deterministic node failure and
MUST NOT invoke the step's tool.

**Outcomes.** Three are recorded distinctly and none is a warning: `passed` (the check exited 0,
or a determination service permitted — the operator's own rules are then still evaluated),
`failed` (the check exited non-zero, or the service denied) — refuse, `error` (timeout, spawn
failure, trust-boundary failure, unreadable artifact, unknown determination vocabulary) — refuse.
There is no "allow on error" mode.

**Fail-closed arming.** Configured-but-unarmed is a refusal, never a silent downgrade: if rules
are configured but cannot arm — malformed configuration, an executable missing at evaluation time
— a matching step is refused with an `unarmed` finding.

**Determinism and domain neutrality.** Evaluation MUST be deterministic: no model, no engine-side
network semantics other than an operator-declared determination service, no retry that changes the
verdict; identical step inputs, configuration and check process MUST produce an identical verdict.
The engine MUST NOT contain domain semantics: a new domain requires a new operator-authored check,
not an engine change. This is demonstrated in case 3 of `docs/spec/validation-protocol-v1.md`.

**Inbound interfaces, both operator-owned.** The check is an argv array (no shell), a JSON stdin
document, documented environment variables, a wall-clock timeout, and exit-code mapping; the check
artifact however invoked — a direct program, or the script an interpreter runs — MUST resolve
outside the agent-writable workspace (RS-025). A precondition MAY instead be satisfied by an
operator-configured external determination service evaluated at the same enforcement point; its
request and response bytes MUST be bound into the record as digests, its endpoint MUST be recorded
redacted, and its determination vocabulary MUST be mapped explicitly — an unknown or newer
vocabulary maps to `error`, and unknown MUST NOT become authority.

**Boundary strength is mode-dependent, and the default row is a permanent caveat** — not a
temporary gap:

| Mode | Guarantee | Does NOT cover |
|---|---|---|
| default install | agent-mediated tool calls cannot touch the check or the authority (path containment + hook) | an unmediated write path to `$HOME` |
| `--isolated` | the agent's UID cannot write the check or the authority (OS ownership and mode) | requires no passwordless privilege escalation for the agent user |
| `require_immutable_check` | a writable check or authority is refused at dispatch | a check writable by a different privileged identity |
| attribution | a forged or changed authority is visible (`authority_digest`) | detection, not prevention |

### L.2 Requirements — which operator obligations does the step leave unmet?

Asked twice from one rule set: when the plan is validated, and again over the prefix as the step
is about to dispatch (RS-030, RS-031). The only outcomes are promote or refuse (RS-033) — a
requirement never rewrites a step — and a promotion is an approval, so it is decided by the
approval binding (§L.5), not by the requirement. A requirement that cannot be evaluated is a
refusal (RS-051), not a pass.

### L.3 Capacity — does the declared cap admit this consumption?

The gate reserves before it authorizes and releases on drop (RS-040, RS-041); a cap that cannot be
read refuses (RS-042) rather than admitting on a zero. Reservation belongs to the caller's dispatch
path — a refusal MUST NOT leave a reservation behind (RS-113), because a refused step that leaves a
hold silently reduces capacity forever.

### L.4 Effects — has this effect already committed?

An effect that has committed MUST NOT be dispatched again, and a retry of an `Ambiguous` effect is
refused (RS-093) — ambiguity is a state the run stays in, not a licence to repeat. The
idempotency key MUST be derived from the intent rather than generated at the call site (RS-094),
so that "the same intent" is a fact the store can decide, and only a resolver presenting provider
evidence may leave `Ambiguous` (RS-096). A deployment MUST NOT claim an effect committed unless its
resolver says so (RS-098).

### L.5 Approval — has a human bound to this consequence signed it off?

The fifth question is not a gate identifier: the answer is a signature bound to the consequence
(ADR-011 canonical intent, HMAC), not an evaluation of the step. A signature is re-derived at
dispatch and a mismatch halts the step; a consequence that reaches its boundary unsigned is refused
as `approval_unbound`, and one whose signature no longer covers it as `approval_expired`. The
approver's identity is attested (RS-013) and a missing key fails closed (RS-014). Because the
binding is a signature and not a finding, it survives a plan change in a way a cached gate verdict
must not (§L.6).

### L.6 Composition and vocabulary

One gate evaluates its questions in a fixed order and answers with one vocabulary; capacity and
commitment are two of the questions, not separate mechanisms, and the wording a refusal carries is
part of the contract because a verifier parses it.

**RS-109** The gate MUST evaluate its questions in a fixed order — preconditions, requirements,
capacity, effects — and MUST stop at the first refusal. Later questions MUST NOT add a finding to a
decision that an earlier question stopped.
*Rationale:* a step that must not run cannot be refused for consuming capacity it never reaches,
and an answer that varies with evaluation order is not reproducible.
*Trace:* `gate/application/compose.rs`; `a_refusal_precondition_is_not_displaced_by_a_requirement_promotion`.

**RS-110** A decision MUST be a pure function of the declared facts, the step, and the instant of
evaluation. No ambient input — wall clock, process environment, filesystem, network, map iteration
order — may reach it.
*Rationale:* a decision that reads its own host is a decision about that host, and two evaluations
of the same inputs must produce the same record.
*Trace:* conformance vectors `vec-0001`, `vec-fixture-0001`; the same digests under a moving clock.

**RS-114** The decision set MUST be the five in the table above and no others. A decision the gate
reaches that is not a row there, or a row there that no execution path reaches, MUST fail
conformance.
*Rationale:* a decision that exists only in the implementation is a consequence nobody reviewed, and
a row naming a decision the code does not reach is a claim the code does not keep.
*Trace:* `engine/src/gate/domain/finding.rs` (`GateId`, four variants) plus the approval binding;
`gate/application/compose.rs`.

**RS-111** A refusal MUST name one kind from a closed vocabulary, and the wire word MUST have exactly
one owner in the implementation. Where a refusal has numbers or identifiers they MUST be carried as
typed fields and the business wording MUST be value-free.
*Rationale:* a verifier parses the word; a sentence that interpolated a deployment's numbers would
put that deployment's data into a static string, where it can be neither redacted nor typed.

| Kind | Wire word |
|---|---|
| precondition failed / errored / timed out | `precondition_failed` · `precondition_error` · `precondition_timeout` |
| unmet or promoted | `requirement_unsatisfied` · `companion_step_missing` · `sequence_violation` |
| history did not verify | `history_integrity_violated` |
| approval | `approval_unbound` · `approval_expired` |
| identity could not be checked | `identity_unverified` |
| a port could not answer, **including a missing operand** | `port_unavailable` |
| the declared scenario is unsupported or sealed | `scenario_unsupported` · `session_sealed` |
| the cap does not admit this consumption | `capacity_exceeded` |
| the effect already committed | `duplicate_effect` |

*Trace:* `wasm/src/domain/outcome/refusal.rs` (`as_str` is exhaustive and equal to serde's spelling).

**RS-112** A missing operand MUST refuse as `port_unavailable`. It MUST NOT be reported as `0`.
*Rationale:* a zero overage is a statement that the request fits; producing it from an unreadable
port is a false statement in the one field an operator would use to size a cap.
*Trace:* `finding_projection.rs`; the `refusaldetail` tests.

**RS-113** A refusal MUST NOT consume capacity or commit an effect. Reservation belongs to the
caller's dispatch path, not to the gate.
*Rationale:* a refused step that leaves a reservation behind silently reduces capacity forever (RS-041).
*Trace:* ADR-018; the dispatcher's reserve-after-permit ordering.
