---
guardian_issue:
  id: "ISSUE-CONSEQUENCE-GATING-SPEC"
  title: "Publish Consequence Gating as a versioned, citable specification with a falsifiable validation protocol and explicit claim ceilings"
  epic: "consequence gating"
  component: "Specification / Architecture"
  module: "consequence-gating"
  status: planned
  priority: high
  dependencies:
    - "ADR-017 consequence gating (the normative content already exists internally)"
    - "ADR-015 operator step requirements"
    - "ADR-016 audit integrity anchor"
    - "1.9.0–1.9.5 release train (behavior is real and installable)"

  intent: |
    Turn the internal architecture artifacts (ADR-017 + module docs + gap ledger)
    into a versioned, externally citable specification with three things we do not
    have today: (1) an explicit jurisdiction statement including non-claims,
    (2) a falsifiable validation protocol with pass/fail criteria, and (3) a
    claim-ceiling model that separates engineering verification from protocol
    validation from independent evidence. The goal is that our public claims can be
    bounded, cited and checked, and cannot be read as broader than they are.

  in_scope:
    - "A versioned specification: jurisdiction, evaluation contract, evidence model, interfaces, failure semantics, reconstructability, non-claims"
    - "An explicit jurisdiction statement: what consequence gating IS and IS NOT responsible for"
    - "A falsifiable validation protocol with stated pass/fail criteria per case"
    - "A domain-neutrality rule and its test: a new domain requires a new operator check, not an engine change"
    - "A claim-ceiling model with three named layers (engineering verification / protocol validation / independent evidence)"
    - "A terminology appendix to prevent vocabulary drift"
    - "A versioning + citation policy"

  out_of_scope:
    - "Certification or compliance claims of any kind"
    - "Proving the operator's authority artifact is externally true"
    - "Claiming to have solved present standing, or to cover every consequence path"
    - "Any cross-vendor integration commitment"

  affected_layers:
    domain:
      - "No code change — this is a specification artifact"
    application:
      - "No code change"
    infrastructure:
      - "New: docs/spec/ directory with the versioned specification + protocol"
    api:
      - "New: a stable document URL/section structure suitable for external citation"

  canonical_references:
    - module: ".pi/architecture/modules/consequence-gating.md"
    - module: ".pi/architecture/modules/precondition.md"
    - adr:
        - ".pi/architecture/decisions/ADR-017-consequence-gating.md"
        - ".pi/architecture/decisions/ADR-015-operator-step-requirements.md"
    - last_sync: "2026-10-12"

  acceptance_criteria:
    - "Specification is versioned and citable (stable identifier + version + date)"
    - "Jurisdiction section states non-claims explicitly, including: identity establishment, delegation legality, domain correctness, truth of observed state, and the downstream system's own commit-point race"
    - "Validation protocol has at least 3 falsifiable cases with explicit pass/fail criteria, each reproducible by an independent reviewer"
    - "The domain-neutrality rule is stated and demonstrated (new domain ⇒ new operator check; engine unchanged)"
    - "Claim-ceiling model names the three layers and states that a passing test does not silently inherit standing at a higher layer"
    - "The authority demo is explicitly classified as fixture-relative engineering evidence, not independent validation"
    - "Terminology appendix defines the terms we overload today (determination, authority, record, verified, evidence)"
    - "Every claim in the spec maps to a reproducible artifact or is marked as non-claim"
    - "FOLDED FINDING A: the evidence-field table defines `check_digest`/`authority_digest` for the provider path, so a reader cannot mistake a provider request hash for a resolved check artifact"
    - "FOLDED FINDING B: the provider body cap is specified as bounding what the engine ALLOCATES, not merely what it records"

  validators:
    - ci
    - architecture
    - canonical

  implementation_notes: |
    Why this is worth doing (context from the interoperability analysis, 2026-10-12):

    We currently publish ADRs — internal decision records — and rely on prose
    caveats to keep claims bounded. That works internally, but it has three
    problems in front of an external reader:

    1. ADRs are not versioned specifications. There is nothing stable to cite, so
       claims float free of a document that bounds them.
    2. The caveats are prose. A caveat that is not a structural part of the artifact
       gets dropped when someone summarizes us.
    3. There is no falsifiable protocol, so an interested third party cannot test
       our claim and cannot tell a demonstration from a validation.

    The public HCS v1.0 series demonstrates the alternative: an
    implementation-independent spec family with explicit jurisdiction (including a
    enumerated list of things the architecture SHALL NOT have authority over), a
    terminology spec whose stated purpose is vocabulary stability, and a separate
    validation protocol that is "falsifiable by design" and whose §12/§13 state
    that engineering tests establish "bounded implementation behavior only" and do
    not "silently inherit standing" at the validation or independent-evidence layer.

    We should adopt that rigor for our own jurisdiction. Not to mirror it — to make
    our claims checkable.

    Hard rules for the author:
    - Non-claims are first-class, enumerated and normative. Silence outside
      jurisdiction must not be readable as implied authority.
    - Every claimed property needs a falsifiable case with a stated failure
      condition. If we cannot state what would disprove it, do not claim it.
    - The three claim-ceiling layers must be explicit and ascending; a lower layer
      never confers a higher one.
    - The authority demo must be re-labelled honestly: it demonstrates behavior
      under a synthetic fixture. It does not establish that any real authority was
      ever valid or invalid. Say so in the artifact, not only in a blog caveat.
    - Ratchet vocabulary: `determination`, `authority`, `evidence`, `verified` are
      all overloaded across our own docs today. The appendix is not cosmetic.

  file_changes:
    - "create: docs/spec/consequence-gating-v1.md"
    - "create: docs/spec/validation-protocol-v1.md"
    - "create: docs/spec/terminology.md"
    - "create: docs/spec/README.md"
    - "create: engine/tests/provider_transport_cap.rs (folded finding B)"
    - "modify: engine/src/precondition/infrastructure/provider_client.rs (folded finding B)"
    - "modify: README.md (link the specification, bound the public claims)"
    - "modify: engine/.pi/architecture/modules/consequence-gating.md (point at the spec)"

  pipeline_steps:
    - implement
    - validate
    - create-mr
    - merge
---

# ISSUE-CONSEQUENCE-GATING-SPEC — make our claims bounded, citable and falsifiable

## Intent

The behavior exists and ships (1.9.0–1.9.5). What does not exist is a **document
that bounds it**. Right now our public claim surface is an ADR, a module doc, a
demo, and a set of prose caveats. An external reader has nothing versioned to cite
and nothing falsifiable to run.

This issue produces three artifacts:

1. **Consequence Gating v1** — a versioned specification: jurisdiction, evaluation
   contract, evidence model, interfaces, failure semantics, reconstructability,
   and non-claims.
2. **Validation Protocol v1** — falsifiable cases with explicit pass/fail criteria.
3. **Terminology** — a normative appendix, because our own vocabulary is overloaded.

## Why now

Interoperability conversations exposed that we argue our position in prose while a
neighboring architecture argues its position in a versioned spec family with an
enumerated jurisdiction and a published falsification protocol. That asymmetry is
not about who is right — it is about who can be **checked** and **cited**.

More concretely, the risk is one-sided: our architecture is strong (dispatch-time
enforcement, operator-owned policy, verifiable records), but our *claims* are
unbounded. "Verified execution", "trustworthy release", "policy-checked" are all
readable as broader than what the system does. A specification with enumerated
non-claims and a claim-ceiling model fixes that structurally, rather than by
remembering to add a caveat each time.

## The claim-ceiling model (the core deliverable)

Three ascending layers. A lower layer never confers a higher one:

| Layer | Establishes | Does NOT establish |
|---|---|---|
| **Engineering verification** | Bounded behavior under a stated fixture and version | Anything about the real world |
| **Protocol validation** | That the specified properties hold across the validated domain mappings | Compliance, certification, fitness |
| **Independent evidence** | Externally attributable facts | That the system is safe to deploy |

The rule to state explicitly: *a passing repository test establishes a bounded
implementation property under its fixture; it does not silently inherit standing
as validation or as evidence of live-environment performance.*

Applied to us honestly: the authority demo is **layer 1**. It demonstrates T₀ →
ΔN → Tₙ refusal under a synthetic fixture. It does not prove the operator's
authority artifact was ever externally true.

## Dependencies

```
┌──────────────────────────────────────┐
│  ADR-017 + ADR-015 + ADR-016         │  EXISTS (internal)
│  released behavior 1.9.0–1.9.5       │  SHIPPED
└───────────────┬──────────────────────┘
                │
                ▼
┌──────────────────────────────────────┐
│  ISSUE-CONSEQUENCE-GATING-SPEC       │ ← THIS ISSUE
│  versioned spec + protocol + ceilings│
└───────────────┬──────────────────────┘
                │
                ▼
┌──────────────────────────────────────┐
│  ISSUE-PRECONDITION-PROVIDERS        │  (interfaces section cites this spec)
│  ISSUE-AUTHORITY-FACT-PROJECTION     │
└──────────────────────────────────────┘
```

## In Scope

- The versioned specification and its jurisdiction/non-claims.
- The falsifiable validation protocol (≥ 3 cases, stated failure conditions).
- The domain-neutrality rule + demonstration.
- The claim-ceiling model.
- The terminology appendix.
- A versioning and citation policy.

## Out of Scope

- Certification, compliance, or security guarantees.
- Proving the operator's authority artifact is true.
- Claiming present standing, or coverage of every consequence path.
- Any vendor integration commitment.

## Affected Layers

This is a documentation artifact; no code changes. The "layers" here are document
structure: specification, protocol, terminology, and index.

## Canonical References

- **Module:** `.pi/architecture/modules/consequence-gating.md`
- **Module:** `.pi/architecture/modules/precondition.md`
- **ADR-017:** `.pi/architecture/decisions/ADR-017-consequence-gating.md`
- **ADR-015:** `.pi/architecture/decisions/ADR-015-operator-step-requirements.md`

## Acceptance Criteria

| # | Criterion | Validator |
|---|-----------|-----------|
| 1 | Versioned + citable (identifier, version, date) | `validate-canonical.sh` |
| 2 | Non-claims enumerated and normative (identity, delegation legality, domain correctness, truth of observed state, downstream commit-point race) | review |
| 3 | ≥ 3 falsifiable cases, each with an explicit failure condition, reproducible by a third party | `validate-ci.sh` |
| 4 | Domain-neutrality rule stated + demonstrated | review |
| 5 | Claim-ceiling model: three layers, no silent inheritance | review |
| 6 | Authority demo classified as layer-1 fixture-relative evidence | review |
| 7 | Terminology appendix resolves our overloaded terms | review |
| 8 | Every claim maps to a reproducible artifact or is marked a non-claim | `validate-canonical.sh` |
| 9 | FOLDED FINDING A — the field table defines `check_digest`/`authority_digest` for a provider (request/response bytes), not only for a check program | review |
| 10 | FOLDED FINDING B — the provider body cap is specified as bounding allocation, not only the recorded evidence | `validate-tests.sh` |

## Folded findings (2026-10-12 verification of PR #1003)

Two defects were found by verifying the implementation against this specification.
Both are folded into this issue rather than filed separately, because both are
*this specification being wrong or silent* rather than missing features:

**Finding A — the evidence table did not define the provider mapping.**
The implementation binds a provider's request bytes as `check_digest` and its
response bytes as `authority_digest` (a deliberate choice to avoid an SDK schema
change). But §3.1 defined those fields only in terms of a *check program* —
"sha256 of the resolved check artifact" — and §4.2 said the bytes "SHALL be bound
into the record as digests" without naming the fields. A reader applying §3.1
would therefore read a provider request hash as "the resolved check artifact".
That is exactly the vocabulary drift this specification and its terminology
appendix exist to prevent.

*Resolution:* §3.1 now defines both fields in terms of the input-side and
authority-side artifact, naming the provider case; §4.2 names the fields it
mandates; the terminology entry for `attribution` states both cases.

**Finding B — "bounded body" was true of the evidence, not of memory.**
The provider client read the response with a full-buffering call and checked the
cap afterwards, so an oversized body was refused *after* being allocated in full.
The module documentation claimed a "bounded body". A provider is an untrusted
input source; the cap must bound allocation.

*Resolution:* the client now rejects on a declared `content_length` and enforces
the cap inside a streaming read loop, so it never buffers beyond the cap. Covered
by `engine/tests/provider_transport_cap.rs` against a real HTTP server (the
provider unit tests inject a stub client and never exercised the transport).

## Implementation

> **Agent:** This is a writing task with a technical standard, not a code task. Read
> ADR-015/016/017 first. The failure mode to avoid is producing a marketing document
> — every property claimed must have a stated way to disprove it.

### Steps

1. Read ADR-015, ADR-016, ADR-017 and both module docs
2. Draft the jurisdiction section with an enumerated non-claims list
3. Draft the evaluation contract from ADR-017 (operator-owned predicate, fail-closed, no LLM)
4. Draft the evidence model (digests, redaction, attribution reasons, anchored mode)
5. Draft the interfaces section (referencing precondition providers and fact projection)
6. Draft the validation protocol with ≥ 3 falsifiable cases and stated failure conditions
7. Draft the claim-ceiling model and re-classify the authority demo
8. Draft the terminology appendix (determination, authority, evidence, verified, record)
9. Add the versioning + citation policy and the index
10. Bound the README's public claims by linking the specification
11. Run validators
12. Create MR
