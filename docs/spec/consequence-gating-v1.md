# Consequence Gating v1.0

| | |
|---|---|
| **Identifier** | `rigorix-consequence-gating` |
| **Version** | `1.0` |
| **Status** | Published |
| **Date** | 2026-10-12 |
| **Normative keywords** | SHALL / SHALL NOT / SHOULD / MAY (RFC 2119) |
| **Companions** | [Validation Protocol v1](./validation-protocol-v1.md) · [Terminology](./terminology.md) · [Index](./README.md) |

This document bounds what Consequence Gating is, what it does, and — normatively
— what it does **not** establish. It is versioned so that a claim can be cited
against a fixed artifact. Prose caveats are not enough: a caveat that is not a
structural part of the document is dropped when the document is summarized.

The behavior specified here ships in Rigorix 1.9.0–1.9.5. This specification adds
no new behavior; it bounds the existing one.

---

## 1. Jurisdiction

Consequence Gating is a **dispatch-time enforcement primitive**: it makes the
dispatch of a consequential step depend on the present standing of that step's
authority, re-read at the moment of consequence, and records the determination in
a signed envelope.

**In jurisdiction.** For a step that matches an operator-authored rule,
Consequence Gating SHALL:

1. evaluate the operator's check at dispatch, immediately before the step's tool
   is invoked;
2. refuse the step when the check does not pass;
3. record the determination — outcome, check/authority digests, attribution — in
   the signed audit record.

**Out of jurisdiction (non-claims).** Consequence Gating SHALL NOT be read as
establishing any of the following. This list is normative; silence outside
jurisdiction is not implied authority.

| # | Non-claim |
|---|-----------|
| NC-1 | **Identity establishment.** It does not establish *who* the operator or the agent is; it evaluates a check the operator authored against identity the operator supplies elsewhere (ADR-011, ADR-015). |
| NC-2 | **Delegation legality.** It does not decide whether a delegation of authority is legally valid. |
| NC-3 | **Domain correctness.** It does not know what "beneficiary eligible" means. The engine SHALL NOT encode domain semantics; the operator supplies the process (ADR-017 §Non-goals). |
| NC-4 | **Truth of observed state.** It records that the check was consulted and what it answered. It does not establish that the operator's authority artifact is externally true. |
| NC-5 | **Closure of the check→action window.** There remains a gap between the check at `T_check` and the action at `T_action`. The downstream system SHALL also enforce at its own commit point. Rigorix does not make the consequence transactional with the check. |
| NC-6 | **The downstream commit-point race.** It does not prevent a change of state between the check passing and the downstream system committing. |
| NC-7 | **Completeness.** It does not claim coverage of every consequence path, only of steps that match a rule. |
| NC-8 | **Certification or safety.** A passing evaluation is not a compliance, security, or fitness-for-purpose claim. |

> **The operative guarantee, stated plainly:** *when the operator's check says no,
> the step is refused, and the determination is signed.* Nothing broader is
> claimed.

---

## 2. Evaluation contract

### 2.1 The primitive

> **match a step → run an operator-authored check → refuse on failure → record
> the outcome.**

Loops, chained checks, retries that change a verdict, a conditionals/policy DSL,
and multi-step orchestration are **not** part of this primitive
(ADR-017 §Non-goals). A use case needing them is a different module.

### 2.2 Order at the choke point

For the node about to dispatch, evaluation SHALL occur in this order:

```
1. approval verification (ADR-011)          verify_before_dispatch
2. precondition evaluation (this spec)      NEW
3. sequence-policy prefix (ADR-013 R3)
4. spawn the node
```

A refusal at step 2 records a deterministic node failure and SHALL NOT invoke the
step's tool.

### 2.3 Determinism

Evaluation SHALL be deterministic: no LLM, no engine-side network semantics other
than an operator-declared determination service, and no retries that change the
verdict. For identical step inputs, config, and check process, the verdict SHALL
be identical.

### 2.4 Outcomes and refusal

Three outcomes SHALL be recorded distinctly:

| Outcome | Cause | Verdict |
|---------|-------|---------|
| `passed` | check exit 0, or a provider determination that permits | the operator's own rules are then still evaluated |
| `failed` | check exited non-zero, or a provider explicitly denied | **refuse** |
| `error` | timeout, spawn failure, trust-boundary failure, unreadable artifact, unknown determination vocabulary | **refuse** |

`failed` and `error` both refuse. There SHALL be no "allow on error" mode in v1.

### 2.5 Fail-closed arming

If rules are configured but cannot arm — malformed configuration, an executable
that is missing at evaluation time — a **matching** step SHALL be refused with an
`unarmed` finding. Configured-but-unarmed SHALL NOT be a silent downgrade.

### 2.6 Domain neutrality

The engine SHALL NOT contain domain semantics. A new domain SHALL require a new
operator-authored check, not an engine change. This is the *domain-neutrality
rule*; it is demonstrated in the [validation protocol](./validation-protocol-v1.md#case-3--domain-neutrality).

### 2.7 Non-inheritance (normative)

When a precondition is satisfied by an operator-configured external determination
service, the provider's permitting answer is **evidence, not authority**. A
permitting provider result SHALL NOT satisfy the step on its own: the operator's
own rules SHALL still be evaluated independently. A denying provider result SHALL
also refuse (the engine does not override the provider either).

---

## 3. Evidence model

### 3.1 The record

Each determination SHALL produce an additive `precondition_findings[]` entry in
the signed audit envelope, carrying at minimum:

| Field | Meaning |
|-------|---------|
| `precondition_id` | the operator-declared rule id |
| `step` | the step the rule matched |
| `outcome` | `passed` \| `failed` \| `error` |
| `exit_code` | the check's exit code, when one ran |
| `inputs_hash` | a digest binding the step inputs the engine fed the check |
| `checked_at` | when the check ran |
| `summary` | a human-readable, redacted summary |
| `check_digest` | sha256 of the artifact the determination was bound to on the *input* side — the resolved check artifact for a check program, or the exact request bytes for a provider (§4.2) (attribution; optional) |
| `authority_digest` | sha256 of the artifact the determination was bound to on the *authority* side — the operator-declared authority artifact for a check program, or the exact response bytes for a provider (§4.2) (attribution; optional) |
| `check_writable` | the boundary fact (attribution; optional) |
| `attribution` | `recorded` \| `refused_before_check` \| `artifact_unreadable` (optional) |

Absent attribution fields mean a pre-attribution envelope — **absent is not
tampered**. A *changed* digest is the signal. Detection only; prevention is the
boundary itself.

### 3.2 SpanPrivacy

The signed record SHALL NOT contain parameter values, raw check stdout, or
absolute paths.

- Parameter presence is recorded (`require_params`), never parameter values.
- Check stdout is recorded only when the operator opts in with
  `capture_output = true`, and is then truncated and redacted.
- **No message that can reach the signed record SHALL carry an absolute path.**
  Each names an artifact by **basename plus a one-way hash of its path**
  (`check.mjs (path sha256:…)`); the full path goes to the operator's local log.
  Redaction SHALL happen where the error is **constructed**, not where it is
  recorded, because a composition root forwards error strings into a fail-closed
  gate's `unarmed_detail` and from there into every refusal.

### 3.3 Reconstruction

A verifier with the published verifiers and (in anchored mode) the anchor
reference SHALL be able to check that the record was not altered after the fact,
without running the engine and without the operator's key. The record SHALL be
sufficient to reconstruct *what was checked, when, with what outcome, at what
chain position* — and SHALL NOT be sufficient to reconstruct *whether the
authority is currently valid*, which is out of jurisdiction (NC-4).

### 3.4 Boundary strength is mode-dependent

The boundary protecting the check and its authority is not a single claim; it is
a **mode → guarantee matrix** (ADR-017 §Honest boundary). The default row is a
permanent caveat, not a temporary gap:

| Mode | Guarantee | Does NOT cover |
|------|-----------|----------------|
| default install | agent-mediated tool calls cannot touch the check or the authority (path containment + hook) | an unmediated write path to `$HOME` |
| `--isolated` | the agent's UID cannot write the check/authority (OS ownership/mode) | requires no passwordless `sudo` for the agent user |
| `require_immutable_check` | a writable check/authority is refused at dispatch | a check writable by a different privileged identity |
| attribution | a forged/changed authority is visible (`authority_digest`) | detection, not prevention |

> **The default install is a policy + path + hook boundary, not a sandbox.**

---

## 4. Interfaces

This section names the seams at which Consequence Gating composes with other
systems. It bounds vocabulary; it does not bind any vendor.

### 4.1 Operator check program (inbound, operator-owned)

An argv array (no shell), a JSON stdin document, documented environment
variables, a wall-clock timeout, and exit-code mapping. The check artifact
however invoked — a direct program, or the script an interpreter runs — SHALL
resolve outside the agent-writable workspace.

### 4.2 External determination service (inbound, operator-owned)

A precondition MAY be satisfied by an operator-configured external service
(HTTP), evaluated at the same enforcement point as the check program. The
service's request and response bytes SHALL be bound into the record as digests —
the request digest as `check_digest` and the response digest as
`authority_digest` (§3.1). For a provider those two fields carry the request and
the response respectively, because there is no local check artifact: the request
is the input-side artifact and the response is the authority-side artifact. The
endpoint SHALL be recorded redacted, and the determination vocabulary SHALL
be mapped explicitly. An unknown or newer vocabulary SHALL map to `error`
(unknown), and unknown SHALL NOT become authority. The provider result enters the
same place an operator check result does; it is governed by §2.7
(non-inheritance).

### 4.3 Authority-fact projection (outbound, ours)

Rigorix MAY export its own hash-chained history as attributable present-state
facts for an external determination consumer. That projection reports *what was
established, when, and how it can be verified*; it SHALL NOT assert present
standing (NC-4). The projection is the producer side of the same seam as §4.2.

The projection SHALL be exported as a **bundle**: the projection, the history it
was derived from, and the subjects the consumer asked about
(`subjects_without_record`, which drives the `NOT_ESTABLISHED` facts and is
required for re-derivation to be reproducible).

#### 4.3.1 Verification SHALL be by re-derivation, not by signature

A verifier SHALL check a bundle by re-deriving it, not by trusting a signature:

1. verify the envelope chain (`verify_chain`) — unverified history re-derives to
   unverified facts, so this is checked first;
2. re-derive the projection from that verified history with the same derivation
   the producer used;
3. compare the **facts** and the **anchor reference** exactly.

The `generated_at` timestamp SHALL NOT be compared: it records when the
projection was produced and is not reproducible. Everything that carries a claim
is compared.

This is a stronger property than a signature over the projection. A signature
proves only that a key holder asserted the artifact; it cannot detect a signer
asserting facts the anchored history does not support. Re-derivation can.

#### 4.3.2 A signed projection is a permitted optimisation, with a condition

A signed (cached) projection MAY be added later to make verification O(1) and to
avoid disclosing the history. If it is added, the signature SHALL be anchored to
the chain: otherwise verification degrades from "consistent with the anchored
chain" to "asserted by a key holder", which is a weaker claim and MUST NOT be
presented as equivalent. The signature is an optimisation in front of the
re-derivation, never a replacement for it.

The disclosure cost is explicit and accepted: a bundle carries the envelopes, so
an isolated signature would avoid handing a third party the full history merely
to answer a question about one authority.

#### 4.3.3 Surface: CLI, not (yet) an MCP tool (decision, 2026-10-13)

The projection SHALL be surfaced by the CLI (`audit projection` /
`audit verify-projection`). It is deliberately NOT exposed as an MCP tool:

- the projection is an EXPORT for an external determination consumer or an
auditor, not a step an agent takes mid-run. Exposing it as a tool widens the
agent-visible surface for something that is fundamentally an offline artifact;
- an MCP tool is a change to the **frozen** method catalog, and that catalog is
cross-repo: the SDK catalog contract SHALL land first, then both servers, then
conformance. A verifier built against the old catalog will not accept the new
method. The CLI already provides the reachability, so the three-repo contract
change is not justified today.

If a tool is added later, the following are fixed in advance so the decision
cannot be reversed silently:

- it is a governed method (`AuthLevel`), named `rigorix.audit.projection`,
following the existing `rigorix.<component>.<verb>` catalog convention;
- its description SHALL state that a bundle carries the **envelopes** — the
caller receives the history, not only a summary. That is inherent to
re-derivation (§4.3.1) and SHALL be declared, not discovered;
- the sequencing is: SDK catalog → OSS server + MCP descriptor/routing →
conformance, in that order.

### 4.4 What these interfaces do NOT do

- They do not transfer constitutional judgment. The exchange of governed state
  does not imply its acceptance.
- They do not make any provider authoritative over Rigorix's own determination.
- They are not a commitment to any vendor or wire format beyond the ones named
  in this version.

---

## 5. Failure semantics

| Situation | Behavior |
|-----------|----------|
| No rules configured | fail-open-absent: today's behavior |
| Malformed / over-cap configuration | **refuse** the affected run (fail closed) |
| Configured but cannot arm | **refuse** a matching step (`unarmed`) |
| Check exit non-zero | **refuse** (`failed`); the tool is never called |
| Check timeout / spawn error | **refuse** (`error`); the tool is never called |
| Check artifact inside the agent-writable workspace | **refuse** (`error`); trust boundary |
| `require_immutable_check=true` and the check/authority is writable or unreadable | **refuse** (`error`); fail closed |
| Provider unreachable / timeout / non-2xx / unparseable / unknown determination | **refuse** (`error`); fail closed |
| Provider permits but our own rule refuses | **refuse** (non-inheritance) |
| Provider denies but our own rule permits | **refuse** (non-inheritance) |
| Non-matching step | unaffected |

---

## 6. Claim-ceiling model

Claims ascend three layers. **A lower layer never confers a higher one.**

| Layer | Establishes | Does NOT establish |
|-------|-------------|--------------------|
| **L1 — Engineering verification** | bounded behavior under a stated fixture and version | anything about the real world |
| **L2 — Protocol validation** | that the specified properties hold across the validated domain mappings | compliance, certification, fitness |
| **L3 — Independent evidence** | externally attributable facts | that the system is safe to deploy |

**Rule (normative):** *a passing repository test establishes a bounded
implementation property under its fixture; it does not silently inherit standing
as validation or as evidence of live-environment performance.*

Applied honestly to this project: the consequence-gating demonstration is
**layer 1**. It demonstrates a refusal under a synthetic fixture. It does not
establish that any real authority was ever valid or invalid.

---

## 7. Versioning and citation

- The identifier `rigorix-consequence-gating` binds this specification family;
  `version` and `date` in the header bind a revision.
- A citation SHALL name the identifier, the version, and a section anchor (for
  example: *Rigorix Consequence Gating v1.0 §2.7*).
- An additive change that does not alter a stated property SHALL increment the
  minor version. A change to jurisdiction, a non-claim, or a failure semantic
  SHALL increment the major version.
- The published verifiers SHALL accept records produced by any `1.x` version; a
  record produced under a later major version MAY be rejected by an earlier
  verifier.

---

## 8. Every claim maps to something reproducible

| Claim | Reproducible artifact |
|-------|-----------------------|
| Refuses on a negative check; tool never called | Validation protocol Case 1 |
| Determinism | Validation protocol Case 2 |
| Domain neutrality | Validation protocol Case 3 |
| Non-inheritance | Validation protocol Case 4 |
| Fail-closed on ambiguity | Validation protocol Case 5 |
| Absent attribution is not tampered | Validation protocol Case 6 |
| No absolute paths in the signed record | Validation protocol Case 7 |
| NC-1…NC-8 (non-claims) | **non-claims** — no artifact is offered, because none is claimed |

---

## References

- **ADRs:** ADR-011 (approval binding), ADR-013 (sequence policy / R5
  `.rigorix/**`), ADR-015 (operator step requirements), ADR-016 (audit integrity
  anchor), ADR-017 (consequence gating).
- **Module doc:** `engine/.pi/architecture/modules/precondition.md`.
- **Validation protocol:** [`validation-protocol-v1.md`](./validation-protocol-v1.md).
- **Terminology:** [`terminology.md`](./terminology.md).
