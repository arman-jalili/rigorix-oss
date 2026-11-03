# ADR-007: Compliance Engine Architecture — We Execute the Engine's Gate, We Do Not Judge Compliance

**Status:** Accepted
**Date:** 2026-11-03
**Session:** 3baf6e5c-e352-4127-af09-a5e7fe0a2d29

## Context

The template asks for a "Compliance Engine Architecture". This crate is adjacent to that
question but must not answer it: it compiles a **slice of the existing engine** so a host
can run the real gate locally. Compliance semantics — which rules exist, what counts as
a violation, who may approve, what a refusal must cite — belong to
`docs/spec/consequence-gating-v1.md` and to the engine's `precondition`,
`sequence_policy`, `approval`, `enforcement`, `identity` and `audit` contexts.

What this crate *does* own is the part a compliance regime actually depends on:

- the gate is evaluated **locally, deterministically and identically** to the reference
  runtime (FR-007);
- a refusal carries a business-worded reason and an evidence reference (FR-002);
- the record can be verified by a third party who trusts neither producer nor host
  (FR-006);
- verification is **three-valued**, so "could not check" never reads as "checked and
  failed" (invariant 6, FR-006);
- artifacts contain no secrets, no host paths and no personal data (NFR-003).

Terminology discipline is part of the architecture: per `docs/spec/terminology.md`,
"verified" means *record-verified* or *check-verified* only, never "the authority is
true" and never "the system is safe". An artifact set that verifies is evidence that the
record was not altered after the fact — it is not a certification.

## Decision

**The seam is an execution and evidence context, not a policy context.**

| Concern | Where it lives |
|---------|----------------|
| Rule evaluation (precondition, sequence, requirement) | `rigorix-engine` — consumed, never re-implemented |
| Approval binding | engine; the crate supplies a *claim*, never a grant (FR-013) |
| Envelope construction and signing | engine; the crate seals and exports |
| Identity attestation | engine (`ring`/identity path excluded from the wasm slice) |
| Verdict + evidence references | engine produces; crate projects without editing |
| Deterministic execution of all of the above | **this crate** |
| Three-valued verification | delegated to `rigorix-verifier` through the bridge |
| Audit-grade byte identity with the CLI | **this crate** (conformance harness) |
| Regulatory interpretation, controls catalogue, retention policy | **out of scope**, host/engine side |

Hard rules that keep the boundary honest:

1. No rule may be expressed in this crate. If a host asks for "one extra check", the
   answer is a scenario change in the engine's format, not a branch here.
2. No message may claim a compliance status ("compliant", "certified", "approved by").
   The vocabulary is: `allowed`, `refused`, `verified`, `not_verified`, `unverifiable`.
3. Refusal reasons are **business-worded projections** of the engine's findings, with the
   evidence reference attached; the crate never softens or re-ranks a finding.
4. A record that cannot be checked is reported as `unverifiable(reason)`; styling it as
   `not_verified` is a defect with a regression test attached.

## Consequences

### Positive
- One source of truth for compliance behaviour: fixing a rule fixes it for browser, CLI,
  MCP and actions at once — and conformance proves it.
- The evidence story survives the producer: an auditor needs the artifact set and the
  verifier, not our code, our server or our key.
- Terminology discipline prevents the most damaging misreading in this domain —
  "verified" as "safe".

### Negative
- The crate inherits the engine's release cadence; a rule change requires an engine
  release before the demo shows it. Intended: it is not a shadow implementation.
- Hosts wanting a compliance dashboard must build it on exported events; we ship no
  reporting surface (ADR-006).

### Neutral
- Spec references are normative elsewhere: `docs/spec/consequence-gating-v1.md`,
  `docs/spec/terminology.md`, `docs/spec/validation-protocol-v1.md`.

## Alternatives Considered

| Alternative | Pros | Cons | Reason Rejected |
|-------------|------|------|-----------------|
| Implement a subset of rules in the seam for browser speed | Fast iteration | Diverges from the CLI; breaks the byte-identical claim and the audit story | The crate's whole value is that it does not diverge |
| Ship a "compliance report" generator | Nice demo output | Invents a claim the engine does not make; invites "verified = compliant" | Terminology rule; not our evidence to assert |
| Verify in-crate instead of via `rigorix-verifier` | No extra dependency | Producer verifying its own output proves nothing | Independence is the invariant |
| Two-valued verification (valid/invalid) | Simpler UI | Accuses a record we could not check | The #472/#1285 rule; FR-006 |

## Implementation

**Affected Modules:** `.pi/architecture/modules/portable-session.md`

**Crate paths:**
- `wasm/src/application/verify_exported_artifacts.rs` — three-valued mapping only
- `wasm/src/infrastructure/verifier_bridge.rs` — the conformist ACL to the SDK
- `wasm/src/domain/artifact/verification_result.rs` — enum with `Unverifiable(reason)`
- `wasm/src/interfaces/messages.rs` — refusal vocabulary allow-list

**Canonical References:** `// @canonical .pi/architecture/decisions/ADR-007-compliance-engine.md`

## Validation

- **security-validator**: no compliance claims in messages; no PII (NFR-003).
- **architecture-validator**: no branch in `domain` that encodes a rule (no policy
  constants; scenario comes from the engine's parser).
- **test-validator**: `unverifiable_never_renders_as_not_verified` and
  `unverified_identity_is_not_unauthenticated` must exist.

## References

- Related ADRs: ADR-003, ADR-004, ADR-005, ADR-010
- Normative specs: `docs/spec/consequence-gating-v1.md`, `docs/spec/terminology.md`

---

*Decision date: 2026-11-03*
*Decision makers: architecture-coordinator, domain exploration session 3baf6e5c*