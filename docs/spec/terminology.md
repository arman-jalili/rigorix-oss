# Terminology (normative appendix v1.0)

| | |
|---|---|
| **Identifier** | `rigorix-consequence-gating-terminology` |
| **Version** | `1.0` |
| **Date** | 2026-10-12 |
| **Specifies** | [`rigorix-consequence-gating` v1.0](./consequence-gating-v1.md) |

This appendix is **normative**. Its purpose is vocabulary stability: the terms
below are overloaded across the project's own documents, and an overloaded term
in a specification is an unbounded claim. Where a term is used in the spec or the
validation protocol, it means what is written here.

## The terms we overload, and their fixed meaning

### authority
The **standing of the world** that a consequential step depends on — for example,
that a beneficiary is presently eligible. *Authority is not the artifact that
records it, and not the check that reads it.* Rigorix does not establish
authority (spec NC-4); it consults an operator artifact and enforces the outcome.

### determination
The **outcome of a check**: `passed`, `failed`, or `error`. A determination is
Rigorix's record of what a check answered, not an assertion about the world. An
external service's answer is also called a determination; when it must be
distinguished, it is the **provider determination**.

### record
The **signed audit envelope** (ADR-016) — hash-chained, and in anchored mode
verifiable without the engine or the operator's key. "Record" never means a log
line or an in-memory value.

### verified
Used in exactly two senses, never a third:

1. **Record-verified** — a published verifier established that a record was not
   altered after the fact.
2. **Check-verified** — an operator check returned `passed`.

"Verified" SHALL NOT be used to mean "the authority is true" or "the system is
safe". A record-verified negative determination is still a refusal.

### evidence
The **facts a record carries**: what was checked, when, with what outcome, and
the digests that bind the artifacts. Evidence is *attributable* — it names its
source — and it is *not* a judgment. Contrast: **judgment** (a determination about
standing) belongs to a jurisdiction; evidence does not transfer judgment.

### precondition
An **operator-authored rule**: a step predicate, an argv check (or a provider
reference), and a failure action. The engine's primitive is *match a step, run a
check, refuse on failure, record the outcome* (spec §2.1). The precondition is the
rule; the **check** is the program or service it names.

### check
The operator's program (argv, no shell) or the operator-configured external
determination service invoked by a precondition. The check is part of the trust
boundary; it must resolve outside the agent-writable workspace.

### provider
An **operator-configured external determination service** (HTTP). A provider
supplies facts and a determination; it does not supply authority (spec §2.7).
"Provider" never implies a vendor endorsement.

### attribution
The optional fields binding a finding to the artifacts it touched: `check_digest`,
`authority_digest`, `check_writable`, and the `attribution` reason
(`recorded` | `refused_before_check` | `artifact_unreadable`). For a check program
the digests bind the resolved check artifact and the authority artifact; for a
provider they bind the request and the response bytes (spec §4.2). Absent
attribution means **pre-attribution**, not tampered.

### standing (present standing)
Whether authority holds **now**. Rigorix does not assert standing (spec NC-1,
NC-4). It reports that a check was consulted and what it answered.

### established
A fact is **established** when a record carries it with provenance (source,
`established_at`, evidence refs). `NOT_ESTABLISHED` means the fact is absent from
the record — it is **not** a refusal and SHALL NOT be rendered as one, nor
promoted to a positive.

### unknown
A determination the engine cannot map to its vocabulary (`passed`/`failed`), or a
state it cannot determine (for example an unreadable artifact). **Unknown is
distinct from adverse** and unknown SHALL NOT become authority (spec §4.2).

### fact / fact projection
A **fact** is an attributable, present-state statement with provenance, exported
by the **fact projection** (spec §4.3). A fact carries *what was established,
when, and how it can be verified*; it does not carry standing.

## Terms deliberately not used

- **"trusted"** — no component is asserted trusted; boundaries are *enforced* and
  *described as a mode → guarantee matrix* (spec §3.4).
- **"guaranteed safe"** — out of scope (spec NC-8).
- **"compliant"** — out of scope.

## The claim-ceiling vocabulary

| Term | Layer | Meaning |
|------|-------|---------|
| engineering verification | L1 | bounded behavior under a stated fixture and version |
| protocol validation | L2 | specified properties hold across the validated domain mappings |
| independent evidence | L3 | externally attributable facts |

A lower layer **never** confers a higher one (spec §6).
