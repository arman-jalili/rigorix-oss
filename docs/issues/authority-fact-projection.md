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
    - "Offline verification with the PUBLISHED verifiers (rust/python/typescript) — no engine needed"
    - "An explicit machine-readable claim ceiling on the projection itself"
    - "Redaction applied at construction (no absolute paths, no usernames, no secrets)"

  out_of_scope:
    - "Asserting current standing (that is the CONSUMER's determination, not ours)"
    - "Becoming an authority source of record for identity or delegation legality"
    - "Any new signing key or trust root (reuse anchored mode)"
    - "Guaranteeing the operator's authority artifact is externally true"

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
    - "Projection is verifiable offline by the published rust/python/typescript verifiers, with no engine and no operator-held key in anchored mode"
    - "Output distinguishes ESTABLISHED from NOT_ESTABLISHED; absence of a revocation record is never reported as non-revocation"
    - "Every fact carries attributable source, established_at and evidence references"
    - "The projection carries an explicit machine-readable claim ceiling (behavior under fixture vs externally proven fact)"
    - "Zero absolute paths / usernames / secrets in the projection (redaction at construction)"
    - "Chain position + anchor reference are included and independently checkable"
    - "A tampered projection is DETECTED by the published verifiers (negative test)"
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
