---
guardian_issue:
  id: "ISSUE-AUTHORITY-ATTRIBUTION"
  title: "Signed attestation: bind the check identity + authority digest into the envelope (OSS + SDK)"
  epic: "authority-boundary"
  component: "PreconditionEvidence"
  status: planned
  priority: high
  order: 4
  dependencies: ["ISSUE-TRUST-BOUNDARY-HARDENING", "rigorix-sdk contract change", "rigorix-enterprise ingestion"]
---

# ISSUE-AUTHORITY-ATTRIBUTION — make a forged authority visible

## Problem

`precondition_findings[].inputs_hash` binds the **step inputs the engine fed
the check** — not the authority the check consulted. So if an agent (or anyone)
writes a permissive `authority.json`, the signed record is unchanged: the
finding still reads `failed`/`exit 3` when it refuses, and nothing records
*what the check actually saw*. Detection is impossible after the fact.

Prevention is issue 02 (isolation). This is **attribution**: make the decision
input tamper-*evident* even when it was writable, so a consumer can tell the
difference between "the operator froze this" and "the file was changed".

## Design (OSS engine)

Add to `PreconditionFinding` / `PreconditionFindingRef` — additive,
`skip_serializing_if`, SpanPrivacy-clean (digests only, never contents):

- `check_digest` — sha256 of the **resolved check program** bytes;
- `authority_digest` — sha256 of the authority artifact the check consumes, when
  the operator declares an `authority_path` on the precondition (the engine
  hashes the file at dispatch; the check does not need to cooperate, and raw
  stdout is **not** promoted into evidence — SpanPrivacy);
- boundary facts from ISSUE-TRUST-BOUNDARY-HARDENING (e.g. `check_writable`).

A forged authority now changes `authority_digest` → the record shows it, and a
verifier can compare against the operator's known-good digest.

## Design (SDK — contract change)

`rigorix-sdk` is byte-exact with the engine, so this is a coordinated change:

- `schemas/envelope.json`: add the properties to `$defs.preconditionFindingRef`
  **in struct order**, and the new `policy.json` `authority_path` key.
- Canonicalization: update `skip_none`/`skipEmpty`/nested maps in **all four**
  verifiers — Go (`go/canonical.go`), Python (`canonical.py`), TypeScript,
  Java — so the HMAC matches the engine.
- Regenerate `_schemas/` + fixtures (`fixtures/envelope/envelope-precondition-signed.json`),
  keep the `corpus_gen.go` mirror in sync, and run the cross-language parity
  suite.
- Extend the precondition fixture (or add one) carrying the new fields.

## Acceptance criteria

| # | Criterion |
|---|-----------|
| 1 | Engine computes and signs `check_digest` + `authority_digest` (+ boundary facts) at dispatch, omitted when absent |
| 2 | Tampering the authority changes `authority_digest`; a test proves the change is visible on the signed envelope |
| 3 | SpanPrivacy: digests only — no contents, no raw stdout |
| 4 | SDK: schema + all four verifiers + fixtures updated; HMAC parity across Go/Python/TS/Java |
| 5 | SDK corpus regenerated from the engine (code is truth) and the parity/drift tests pass |
| 6 | Absent fields = pre-attribution envelope (additive; absent ≠ tampered) |
| 7 | Enterprise ingestion of the new fields tracked in the enterprise issue |
| 8 | When this lands, flip the **attribution** row in the ADR-017 boundary matrix (#984) from *planned* → *available* |

## Files

- `engine/src/precondition/domain/finding.rs`, `domain/precondition.rs` (`authority_path`)
- `engine/src/audit/domain/envelope.rs` (`PreconditionFindingRef`)
- `engine/src/audit/application/envelope_factory_impl.rs` (derivation)
- `rigorix-sdk/schemas/envelope.json`, `policy.json`, all verifiers, fixtures
- `rigorix-enterprise` ingestion (cross-repo)

## References

- ISSUE-BOUNDARY-HONESTY (#2: inputs_hash ≠ authority)
- ISSUE-AUTHORITY-ISOLATION (prevention; this is detection)
- ADR-016 (the anchor the attributed finding rides)
