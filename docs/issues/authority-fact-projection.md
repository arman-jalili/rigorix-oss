---
guardian_issue:
  id: "ISSUE-AUTHORITY-FACT-PROJECTION"
  title: "Authority-fact projection: a third-party-verifiable export of signed history as attributable present-state facts"
  epic: "consequence gating"
  component: "Audit / Evidence"
  module: "audit-integrity"
  status: planned
  priority: medium
  dependencies:
    - "ADR-016 audit integrity anchor (anchored mode)"
    - "ADR-017 evidence redaction (no absolute paths in signed records)"
    - "SDK 0.4.0 published verifiers (rust / python / typescript)"
    - "ISSUE-PRECONDITION-PROVIDERS (consumer of this output)"

  intent: |
    Produce a signed, offline-verifiable projection of Rigorix's own hash-chained
    history as attributable present-state facts: what authority was checked, when,
    with what outcome, with what digests, and at what chain position. The purpose is
    to be the PRODUCER of verifiable facts for an external determination consumer —
    a consumer that is explicitly forbidden from treating its own runtime packet as
    provenance — without asserting present standing itself.

  in_scope:
    - "A projection command/API: given identity + scope + run/time window, emit a fact set"
    - "Facts carry provenance: attributable source, established_at, evidence refs, epistemic status"
    - "Chain position + anchor reference so a third party can verify lineage"
    - "A bundle: the projection + the history it was derived from + the subjects asked about"
    - "Verification by RE-DERIVATION (chain-verify, re-derive, compare facts and anchor)"
    - "An explicit machine-readable claim ceiling on the projection itself"
    - "Redaction applied at construction (no absolute paths, no usernames, no secrets)"

  out_of_scope:
    - "Asserting current standing (that is the CONSUMER's determination, not ours)"
    - "Becoming an authority source of record for identity or delegation legality"
    - "Any new signing key or trust root (reuse anchored mode)"
    - "Guaranteeing the operator's authority artifact is externally true"
    - "A SIGNED/cached projection — permitted later, but ONLY if anchored to the chain. Deferred, not rejected: see the design note in the body."

  affected_layers:
    domain:
      - "New: `AuthorityFact` (subject, scope, established_at, epistemic_status, evidence_refs)"
      - "New: `ClaimCeiling` — what the projection does and does not establish"
      - "Modified: anchor projection types to expose chain position"
    application:
      - "New: ProjectAuthorityFacts use case (identity + scope + window -> fact set)"
      - "Modified: audit application service to expose the projection"
    infrastructure:
      - "Modified: anchored history reader (chain position + anchor ref)"
      - "Modified: serialization to reuse the canonical (verifier-compatible) form"
    api:
      - "New: CLI command + MCP tool surface for the projection"
      - "New: documented JSON shape, verified offline by the published verifiers"

  canonical_references:
    - module: ".pi/architecture/modules/consequence-gating.md"
    - adr:
        - ".pi/architecture/decisions/ADR-016-audit-integrity-anchor.md"
        - ".pi/architecture/decisions/ADR-017-consequence-gating.md"
    - last_sync: "2026-10-12"

  acceptance_criteria:
    - "The projection + its history round-trip as a bundle (JSON)"
    - "An intact bundle verifies by re-derivation; a forged digest, a dropped fact, an omitted asked-about subject and a tampered anchor are each DETECTED"
    - "A broken chain (broken link, interior deletion) is rejected BEFORE re-derivation — unverified history yields no trustworthy facts"
    - "Output distinguishes ESTABLISHED from NOT_ESTABLISHED; absence of a revocation record is never reported as non-revocation"
    - "Every fact carries attributable source, established_at and evidence references"
    - "The projection carries an explicit machine-readable claim ceiling (behavior under fixture vs externally proven fact)"
    - "Zero absolute paths / usernames / secrets in the projection (redaction at construction)"
    - "The verification procedure is specified (spec 4.3.1) and the signed-cache alternative is recorded with its anchoring condition (spec 4.3.2)"
    - "A CLI command emits a bundle; the MCP tool is deferred pending a frozen-catalog change"
    - "validate-ci.sh / validate-tests.sh / validate-security.sh / validate-canonical.sh pass"

  validators:
    - ci
    - tests
    - security
    - architecture
    - canonical

  implementation_notes: |
    This is the PRODUCER side of the precondition-provider seam, and it is the half
    that does not require any change on a third party's side. The reasoning:

    An external determination consumer needs attributable present-state facts and is
    explicitly required not to treat its own runtime state as evidentiary
    provenance. Rigorix already produces exactly that: hash-chained, signed records
    that a third party can verify without running the engine. So the useful move is
    not to ask for a receipt — it is to hand over verifiable facts.

    Hard rules:
    - Do not assert standing. Say "authority X was checked at T with outcome O and
      digest D, at chain position N, anchored by A". Never "authority X is valid".
    - Preserve UNKNOWN as distinct from adverse. `NOT_ESTABLISHED` is not a refusal
      and must never be rendered as one, and must never be promoted to a positive.
    - Absence of evidence of revocation is NOT evidence of non-revocation. Encode
      that in the shape, not just the docs.
    - Carry the claim ceiling as data, not prose. A consumer should be able to read
      the ceiling programmatically.
    - Reuse the ADR-017 redaction helpers; the projection is shared with auditors,
      so a username or directory layout leak is a real disclosure.

  file_changes:
    - "create: engine/src/audit/domain/authority_fact.rs"
    - "create: engine/src/audit/application/projection.rs"
    - "modify: engine/src/audit/infrastructure/anchor.rs"
    - "create: engine/tests/unit/audit/authority_fact_test.rs"
    - "create: engine/tests/integration/authority_fact_projection.rs"
    - "modify: cli/src/cli_boundary/orchestrator.rs"
    - "modify: mcp/src/audit_tools/domain/formatter_impl.rs"
    - "modify: engine/.pi/architecture/modules/consequence-gating.md"

  pipeline_steps:
    - implement
    - validate
    - create-mr
    - merge
---

# ISSUE-AUTHORITY-FACT-PROJECTION — our records as attributable facts

## Intent

Rigorix already holds the thing an external determination consumer says it needs:
signed, hash-chained records that a third party can verify **without running the
engine and without the operator's key** (anchored mode, ADR-016). This issue
exposes that as a deliberate output — a *fact projection* — rather than leaving it
as an internal audit trail.

The strategic point: the counterpart's own contract requires verifiable provenance
and forbids it from treating a runtime packet as its own evidence. We are a
legitimate source of such evidence for authority state specifically. That makes us
the producer of its input, which is a stronger and simpler position than asking it
to route execution through us.

## Why not assert standing

Because asserting standing is not our jurisdiction and would be an overclaim we
could not defend. Our claim is narrower and stronger:

> "At chain position N, authority X for scope S was checked at time T, the check
> artifact had digest D, and the outcome was O. Here is the anchor that lets you
> verify none of that was altered."

Whether that constitutes *present* standing is the consumer's determination. We
supply facts with provenance; they supply the judgement. That split is the same
discipline that already governs our own evidence (the `recorded` /
`refused_before_check` / `artifact_unreadable` distinction exists precisely so that
absences are not overloaded).

## Design decision — verification by re-derivation, not by signature

Two designs were considered for making the projection checkable offline.
**Design 1 (re-derivation) is implemented; design 2 (signing) is documented here as
a deferred optimisation, not a rejected idea.**

| | Design 1 — re-derivation (chosen) | Design 2 — signed projection (deferred) |
|---|---|---|
| What it proves | "this is what the **anchored history** says" | "the holder of **key K** asserts this" |
| Verifier work | re-run the derivation | canonicalize + `verify_ed25519` (already in all 5 languages) |
| Artifact size | the envelopes (large) | a small self-contained artifact |
| Disclosure | the full history slice | just the summary |
| Verification cost | O(n) over envelopes | O(1) |
| SDK change | none | a new frozen canonical form + a `verify_*` |

**Why design 1 wins for now.** A signature cannot detect a signer asserting facts
the chain does not support — it answers *who said it*, not *whether it is so*. If
the projection is signed by the operator's engine key (which the operator holds),
then the operator can emit a projection that is not derivable from the chain and
verification still passes. Anchored mode's entire premise is that the anchor key
is one the operator does **not** hold, so reaching design 1's strength by signing
would require the anchor service to sign projections — a contract change on the
anchoring side, plus a new frozen canonical form. That is the exact coordination
the 0.3.0→0.4.0 sequence cost us once already.

**Real advantages of design 2, which are why it stays on the table.** It matches
the SDK's established shape (`verify_ledger_receipt`, `verify_history_slice`,
`verify_compaction_checkpoint` are all typed signed artifacts with a dedicated
`verify_*`); it is O(1) instead of O(n); and it does not hand a third party the
entire history merely to answer a question about one authority. Design 1's
disclosure cost is real and is accepted explicitly.

**The condition if design 2 is ever added** (recorded in spec §4.3.2): the
signature SHALL be anchored to the chain, and it is an optimisation *in front of*
re-derivation — never a replacement for it. Unanchored, it would silently
downgrade the guarantee from "consistent with the anchored chain" to "asserted by
a key holder", which must not be presented as equivalent.

**What this means for the original AC.** The AC said "verifiable offline by the
published verifiers". The published verifiers verify *envelopes and typed signed
artifacts*; none of them derives a projection, so that AC could not be satisfied
as written without either publishing the derivation to every language or adding a
signed artifact type. The AC is now: verify by re-derivation (engine-side, no SDK
change), with the signed variant specified and conditionally permitted.

## Dependencies

```
┌──────────────────────────────────────┐
│  ADR-016 anchor + hash-chained audit │  EXISTS
│  offline verifiers (rust/py/ts)      │  PUBLISHED
└───────────────┬──────────────────────┘
                │
                ▼
┌──────────────────────────────────────┐
│  ISSUE-AUTHORITY-FACT-PROJECTION     │ ← THIS ISSUE (producer)
└───────────────┬──────────────────────┘
                │  attributable present-state facts
                ▼
┌──────────────────────────────────────┐
│  ISSUE-PRECONDITION-PROVIDERS        │  (consumer side)
└──────────────────────────────────────┘
```

## In Scope

- A projection for identity + scope + time window.
- Provenance on every fact: attributable source, established_at, evidence refs.
- Epistemic status that preserves UNKNOWN distinctly.
- Chain position + anchor reference (independently checkable).
- Offline verification by the published verifiers.
- A machine-readable claim ceiling.
- Redaction at construction.

## Out of Scope

- Asserting present standing.
- Becoming an authority of record for identity or delegation legality.
- New keys or trust roots.
- Guaranteeing the operator's authority artifact is externally true — we record
  what was checked, not that the world matches it.

## Affected Layers

### Domain
- New: `AuthorityFact`, `ClaimCeiling`
- Modified: anchor projection types (expose chain position)

### Application
- New: `ProjectAuthorityFacts`
- Modified: audit application service

### Infrastructure
- Modified: anchored history reader
- Modified: canonical serialization (must stay verifier-compatible)

### API
- New: CLI command + MCP tool, documented JSON shape

## Canonical References

- **Module:** `.pi/architecture/modules/consequence-gating.md`
- **ADR-016:** `.pi/architecture/decisions/ADR-016-audit-integrity-anchor.md`
- **ADR-017:** `.pi/architecture/decisions/ADR-017-consequence-gating.md`
- **Verifiers:** `rigorix-sdk` rust / python / typescript (published 0.4.0)

## Acceptance Criteria

| # | Criterion | Validator |
|---|-----------|-----------|
| 1 | Verified offline by published verifiers, no engine, no operator key | `validate-integration.sh` |
| 2 | ESTABLISHED vs NOT_ESTABLISHED distinct; absence of revocation ≠ non-revocation | `validate-tests.sh` |
| 3 | Every fact has source, established_at, evidence refs | `validate-tests.sh` |
| 4 | Claim ceiling is machine-readable | `validate-tests.sh` |
| 5 | Zero absolute paths / usernames / secrets | `validate-security.sh` |
| 6 | Chain position + anchor ref independently checkable | `validate-integration.sh` |
| 7 | Tampered projection is DETECTED (negative test) | `validate-tests.sh` |
| 8 | Canonical + architecture layers respected | `validate-canonical.sh` |

## Implementation

> **Agent:** Read ADR-016 and ADR-017 first. The rule that matters most is:
> **the projection must never assert present standing.** It reports what was
> established, when, and how it can be verified — nothing more.

### Steps

1. Read ADR-016 (anchored mode), ADR-017 (evidence redaction), and the canonical serialization
2. Define `AuthorityFact` and `ClaimCeiling` in the domain
3. Implement the projection use case (identity + scope + window)
4. Expose chain position and anchor reference
5. Serialize in the verifier-compatible canonical form
6. Apply redaction at construction (basename + one-way hash)
7. Write positive tests (verifiers accept) and negative tests (tampering detected)
8. Expose via CLI + MCP surface
9. Run all validators
10. Create MR
