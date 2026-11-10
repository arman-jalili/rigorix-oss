# Consequence Gating v2.0

**Identifier:** `rigorix-consequence-gating`
**Version:** 2.0
**Date:** 2026-11-10
**Status:** Published.
**Supersedes:** [Consequence Gating v1.0](./consequence-gating-v1.md). v1.0 remains published and
immutable: a claim about the precondition gate alone may still cite it. Where the two differ, this
document is the contract.
**Implements:** ADR-017 (consequence gating), ADR-018 (distributed capacity ledger), ADR-019 (effect
commitment), ADR-022 (the binding boundary), ADR-023 (the sink boundary).

---

## 1. Jurisdiction

v1.0 specified one gate: a step matched a precondition, an operator-authored check ran, and a
negative answer refused the step. v2.0 specifies the **composed gate** — four gates in one fixed
order — and the **artifact form** in which the gate ships to a host that is not the engine's own
process.

Three things do not change from v1.0, and are restated here because a version bump invites the
assumption that they did:

1. **The wire strings are frozen.** `precondition_checked`, `precondition_findings[]`,
   `precondition_denied`, and the configuration path `.rigorix/preconditions.toml` keep the
   spelling v1.0 gave them. A v2 host reads a v1 record without translation.
2. **`failed` and `error` both refuse**, and there is no "allow on error" mode (§5).
3. **Non-inheritance** (v1.0 §2.7) applies to every gate, not only to preconditions: a permitting
   answer from an external system is evidence, never authority (§2.6).

### 1.1 What a v2 deployment is

A deployment that evaluates the composed gate and produces a decision record. It may evaluate the
gate in-process (the artifact of §4), in the engine's own process, or by calling a host that does.
The specification binds the *decision and its record*, not the transport.

---

## 2. The composed gate

### 2.1 The order is normative

```text
1. preconditions   — may the step dispatch at all?          ADR-017
2. requirements    — which operator obligations does it leave unmet?   ADR-013 R9 / ADR-015
3. capacity        — does the declared cap admit this consumption?     ADR-018
4. effects         — has this effect already committed?                ADR-019
```

The walk stops at the first gate that refuses (fail closed). Findings are returned in this order.

**3 and 4 are appended, not interleaved.** A composition that produced findings before capacity and
effects existed SHALL produce the same bytes now: the two later gates add no finding to a run that
stopped earlier. They are evaluated **only when the verdict still permits dispatch** — a step that
must not run cannot be refused for consuming capacity it never reaches.

Preconditions run first because they answer a different question. A precondition is about the
step's consequence (*this must not happen*); a requirement is about whether the plan may proceed
with this step in it (*this is unmet*), and can promote a step to human approval. Promoting a step
that will never dispatch is evidence about a run that cannot happen.

### 2.2 Facts in, decision out

A decision SHALL be a pure function of:

- the declared world (the scenario, its rules, its caps, its prior commitments),
- the step (name, tool, parameters),
- the **instant** at which it is evaluated,

and of nothing else. No ambient state SHALL enter: no wall clock, no process environment, no
filesystem read, no network, no iteration order of an unordered map. Two evaluations of the same
three inputs SHALL produce byte-identical records (§8, vec-0001).

This is why the gate can be a wasm artifact at all: a decision that read its own host would be a
decision about that host.

### 2.3 Gate identities

Each finding carries the gate that produced it, as a stable string:

| Gate | String |
|---|---|
| preconditions | `preconditions` |
| requirements | `requirements` |
| capacity | `capacity` |
| effects | `effects` |

### 2.4 Capacity (gate 3)

A consumption is admitted only if the declared cap admits it. The arithmetic is the world's, not the
host's:

```text
claimed  = committed + outstanding + requested
admitted = claimed <= hard_limit
```

A refusal states the four operands in the record (§2.6). The specification does not fix where
`committed`/`outstanding` come from — a file, a database, a contested store — only that the numbers
in the record are the numbers the decision used, and that a decision does not invent an operand.

### 2.5 Duplicate effect (gate 4)

An effect that has already committed SHALL NOT be dispatched again. The gate reports the prior
commitment's **record identity** and the **instant** it happened, so that the refusal is checkable
against the store that holds it.

An absent record is not a duplicate: absence is free, and a gate that refused on absence would
refuse every first attempt.

### 2.6 Refusals

A refusal SHALL name one kind, from a closed vocabulary. The wire word is part of the contract —
`serde`'s spelling and the hand-written accessor SHALL be equal, and both are exhaustive, so a
sixteenth kind cannot be added without deciding what a host reads.

| Kind | Wire word | Produced by |
|---|---|---|
| `PreconditionFailed` | `precondition_failed` | gate 1: a check ran and answered no |
| `PreconditionError` | `precondition_error` | gate 1: the check could not run |
| `PreconditionTimeout` | `precondition_timeout` | gate 1: the check exceeded its budget |
| `RequirementUnsatisfied` | `requirement_unsatisfied` | gate 2: an operator obligation is unmet |
| `CompanionStepMissing` | `companion_step_missing` | gate 2: a required companion had not run |
| `SequenceViolation` | `sequence_violation` | gate 2: the order the policy requires was violated |
| `HistoryIntegrityViolated` | `history_integrity_violated` | gate 2: the history the policy reads did not verify |
| `ApprovalUnbound` | `approval_unbound` | gate 2: an approval does not bind to this intent |
| `ApprovalExpired` | `approval_expired` | gate 2: an approval bound, but had expired |
| `IdentityUnverified` | `identity_unverified` | gate 2: identity could not be checked |
| `PortUnavailable` | `port_unavailable` | any gate: a port could not answer, including a **missing operand** |
| `ScenarioUnsupported` | `scenario_unsupported` | the declared format version is not supported |
| `SessionSealed` | `session_sealed` | the session was sealed; nothing may be appended |
| `CapacityExceeded` | `capacity_exceeded` | gate 3: the cap does not admit this consumption |
| `DuplicateEffect` | `duplicate_effect` | gate 4: the effect already committed |

Two rules about meaning, both normative:

- **`IdentityUnverified` is never rendered as "unauthenticated".** "We could not check" SHALL NOT
  read as "it is wrong". The same discipline applies to every kind whose subject is a failure to
  determine: the wording names the failure, not a conclusion.
- **A refusal consumes nothing.** A step refused at gate 1 or 2 SHALL NOT have committed capacity
  or an effect. Reservation happens at dispatch, in the caller's path, not in the gate.

### 2.7 Operands travel as fields, never in the sentence

Where a refusal has numbers or identifiers, they SHALL be carried as typed fields (a `detail`
object), and the business wording SHALL be **value-free**. A sentence that quoted a host's numbers
would put that host's data into a static string, where it cannot be redacted and cannot be typed.

```text
Capacity        { requested, claimed, hard_limit, overage }
DuplicateEffect { record_id, instant }
```

The subject of a `detail` field is the refusal it accompanies: `CapacityExceeded` carries the
capacity operands, `DuplicateEffect` the duplicate's record identity and instant.

**A missing operand SHALL refuse as `PortUnavailable`, and SHALL NOT be reported as `0`.** A zero
overage is a statement that the request fits; producing it from an unreadable port is a false
statement in the one field an operator would use to size the cap.

### 2.8 Non-inheritance, generalised

When a gate consults an external determination (a provider, a ledger, a commitment store), its
answer is evidence, not authority. A permitting answer SHALL NOT satisfy the step on its own: the
operator's own rules are still evaluated. A denying answer SHALL also refuse. Neither party
overrides the other.

---

## 3. Evidence

### 3.1 The record

A decision record carries the verdict, the gate that produced it (when refused), the findings in
composition order, and the evidence each finding cites. Evidence kinds are closed:

| Evidence kind | What it is |
|---|---|
| `precondition` | a check's outcome, its artifact digest, its attribution |
| `sequence_policy` | an operator requirement and whether it was met |
| `requirement` | a companion or ordering obligation |
| `approval` | a bound approval: intent, approver handle, expiry |
| `identity` | an attestation, or the reason there is none |
| `port_failure` | a port that could not answer |
| `capacity` | the four operands of §2.4 |
| `effects` | the prior commitment a duplicate names |

An evidence kind SHALL NOT be added without a specification version that names it and a conformance
vector that exercises it (§8).

### 3.2 Completeness is a state, not a log line

A run's evidence is one of:

| Status | Meaning |
|---|---|
| `complete` | every decision in the run has an evidence record that was emitted and, where configured, delivered |
| `degraded` | the run completed, but at least one record was not emitted or not delivered; the status carries the reason and the count |
| `not_requested` | evidence was not configured for this run |

A degraded run SHALL NOT be presented as complete, and the reason SHALL be a field, not a warning on
stderr. "The run succeeded and the evidence for it did not" is a fact about the run.

### 3.3 What the record does not prove

The record proves what was decided, under which declared facts, at which instant, and that it has
not changed since (V1–V3, v1.0 §3.2). It does not prove:

- that the declared world was true,
- that the operator's check was correct,
- that the consequence did not happen anyway,
- that the host that produced it was uncompromised.

---

## 4. The artifact surface

A host that is not the engine's own process SHALL be able to evaluate the gate with no network, no
filesystem and no engine build. The reference form is one WebAssembly module built for
`wasm32-wasip1` (ADR-022).

### 4.1 Three exports, one entry

```text
rigx_alloc(len: u32) -> ptr        the host asks for a request buffer
rigx_call(ptr: u32, len: u32) -> ptr   ONE entry; the operation is named inside the JSON
rigx_free(ptr: u32, len: u32) -> ()    the host returns a buffer, or frees a response
```

The operation SHALL be a field inside the request JSON, not a separate export. One entry with a
named operation cannot grow a surface by accident; a fifth export can.

A response SHALL be length-prefixed and SHALL NOT be trusted by the host without checking the
prefix against the buffer it was given (§8, hostile corpus).

### 4.2 Six imports, five behaviours

A `wasm32-wasip1` build links six WASI prologue functions, whether or not the module calls them:

```text
environ_get  environ_sizes_get  clock_time_get  random_get  fd_write  proc_exit
```

**No decision reads any of them.** This is the provable claim, and the one a security questionnaire
can check: the module's import section states what it *can* reach, and the recorded vectors
reproduce with every one of those six answered by a constant.

A module whose import section is not exactly this set SHALL be refused at load, naming what is
missing and what is unexpected. An unused capability is still a capability.

### 4.3 Load-time refusal

Before instantiating, a host SHALL:

1. refuse a module whose import section is not the recorded six (§4.2);
2. refuse a module whose bytes are not the recorded digest, naming both digests.

A byte-identical rebuild of the same commit SHALL produce the same digest **on the same toolchain**;
a Linux build and a macOS build of the same commit are different artifacts, and a digest SHALL be
published with the platform and commit it belongs to. A digest without that context pins nothing.

### 4.4 The embedder's duty

The import section says what the module *can reach*. It does not say what the host *grants*: a WASI
runtime will happily supply the caller's environment, wall clock, entropy and stdout. A conforming
embedder SHALL answer:

| Import | Answer |
|---|---|
| `environ_get`, `environ_sizes_get` | `0`, writing nothing |
| `clock_time_get` | `0`, and count the call |
| `random_get` | leave the (already zero) buffer alone, and count the call |
| `fd_write` | discard, and count the bytes |
| `proc_exit` | **raise**, never return |

The counts SHALL be maintained and SHALL be zero for the clock and for stdout on a passing run. A
monitor is not a fix: a non-zero count is a finding, not a warning.

### 4.5 Who is responsible for what

| Property | Owner |
|---|---|
| memory safety inside the sandbox | the runtime (wasmtime, wazero, …) |
| which capabilities the module is given | the embedder's import table |
| the correctness of a decision | this specification, and its vectors |
| that the file is the artifact that was built | the published digest, on the release page |

A host that answers the six with constants and verifies the digest has moved every property it can
to a checkable one. A host that skips either has not.

---

## 5. Failure semantics

| Situation | Behaviour |
|---|---|
| No rules or caps configured | fail-open-absent: nothing is evaluated for that gate |
| Malformed or over-cap configuration | **refuse** the affected run (fail closed) |
| Configured but cannot arm | **refuse** a matching step (`unarmed` finding) |
| Check exits non-zero | **refuse** (`precondition_failed`); the tool is never called |
| Check times out or cannot spawn | **refuse** (`precondition_timeout` / `precondition_error`) |
| Check artifact inside the agent-writable workspace | **refuse** (`error`); trust boundary |
| A port cannot answer | **refuse** (`port_unavailable`); never a zero operand |
| Declared cap does not admit the consumption | **refuse** (`capacity_exceeded`) with the four operands |
| The effect already committed | **refuse** (`duplicate_effect`) with the prior record and instant |
| Provider permits, our own rule refuses | **refuse** (non-inheritance) |
| Provider denies, our own rule permits | **refuse** (non-inheritance) |
| Non-matching step | unaffected |
| A step that may execute a program has no declared isolation | **refuse** (`sandbox_undeclared`) **before any spawn**; see `execution-governance` RS-099 |
| A permission-mode value is unrecognized | the host **refuses to start**; no permissive fallback |
| Evidence was not delivered | the run reports `degraded` with the reason; nothing reports success |

---

## 6. Claim-ceiling model

The three layers of v1.0 §6 are unchanged, and one row is added for the artifact:

| Layer | Establishes | Does NOT establish |
|---|---|---|
| **L1 — Engineering verification** | bounded behaviour under a stated fixture and version | anything about the real world |
| **L2 — Protocol validation** | that the specified properties hold across the validated domain mappings | compliance, certification, fitness |
| **L3 — Independent evidence** | externally attributable facts | that the system is safe to deploy |

Applied to the artifact: *"the module imports six functions and no decision reads them"* is **L1**
for the recorded vectors, under the recorded digest. It SHALL NOT be cited as evidence that a
particular deployment's host answered them with constants — that is the embedder's claim, and it is
checkable by reading that embedder's import table.

---

## 7. Versioning and citation

- The identifier `rigorix-consequence-gating` binds the family; `version` and `date` bind a revision.
- A citation SHALL name the identifier, the version and a section anchor (*Rigorix Consequence
  Gating v2.0 §2.7*).
- An additive change that alters no stated property increments the minor version. A change to
  jurisdiction, to a non-claim, or to a **failure semantic** — including adding or removing a
  refusal kind, an evidence kind or a gate — increments the major version. v1.0 → v2.0 is the first
  such change: gates 3 and 4 are new failure semantics and the artifact surface is new jurisdiction.
- The published verifiers SHALL accept records produced by any `2.x` version. A `1.x` verifier MAY
  reject a `2.x` record (it has no kind for `capacity_exceeded`); a `2.x` verifier SHALL accept a
  `1.x` record, because no v1 wire string changed.

---

## 8. Every claim in this document maps to something reproducible

| Claim | Reproducible artifact |
|---|---|
| The four gates compose in this order and stop at the first refusal | `engine/src/gate/application/compose.rs`; `a_refusal_precondition_is_not_displaced_by_a_requirement_promotion` |
| A decision is a pure function of world + step + instant | vectors `vec-0001`, `vec-fixture-0001`; the same digests under a moving clock (§4.4) |
| A cap refusal carries its four operands | `vec-0002` (cap 1 420 000, committed 1 409 000) |
| A duplicate refusal carries the prior record and instant | `vec-0003` (effect `QR-2041`) |
| A missing operand is `port_unavailable`, never `0` | `wasm/src/domain/outcome/finding_projection.rs`; the `refusaldetail` tests |
| Fifteen refusal kinds, one wire word each | `wasm/src/domain/outcome/refusal.rs` (`as_str` is exhaustive) |
| The artifact imports six and reads none | `node wasi/ci/embedder-contract.mjs --conformance <artifact>` → `clock reads 0`, 5/5 digests |
| A seventh import is refused at load | `wasi/ci/embedder-contract.mjs --self-test` (seed S4) |
| A modified artifact is refused at load | the Go and Python bindings' digest tests |
| The artifact does not ship the builder's filesystem | `bash wasi/ci/check-no-builder-paths.sh` |
| The published artifact is the built one | `gh release view <tag> --json assets`; `wasi/ci/check-release-asset.sh <tag> --expect <digest>` |

---

## References

- **ADRs:** ADR-017 (consequence gating), ADR-018 (distributed capacity ledger), ADR-019 (effect
  commitment), ADR-022 (the binding boundary), ADR-023 (the sink boundary).
- **Predecessor:** [Consequence Gating v1.0](./consequence-gating-v1.md) — the precondition gate,
  the wire strings, the three outcome classes.
- **Requirements:** [`execution-governance/03-requirements.md`](./execution-governance/03-requirements.md)
  (RS-099–RS-107 are the boundary obligations this document relies on).
- **Validation protocol:** [`validation-protocol-v1.md`](./validation-protocol-v1.md).
- **Terminology:** [`terminology.md`](./terminology.md).
