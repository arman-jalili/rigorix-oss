---
guardian_issue:
  id: "ISSUE-ATTRIBUTION-ABSENCE-REASON"
  title: "Attribution absence has three causes and one empty state — add a structured reason code"
  epic: "consequence-gating"
  component: "PreconditionFinding"
  status: planned
  priority: high
  dependencies: ["rigorix-sdk contract change (next release)"]
  blocking: ["rigorix-enterprise console attribution rendering"]
---

# ISSUE-ATTRIBUTION-ABSENCE-REASON — absent ≠ pre-attribution

## Problem

`precondition_findings[]` carries `check_digest`, `authority_digest` and
`check_writable` (#987, 1.9.2), each `skip_serializing_if Option::is_none`. That
made "absent" overloaded — it now means **three different things**, and a
consumer cannot tell them apart:

| Cause | What happened | What a verifier should conclude |
|---|---|---|
| **pre-attribution engine** | The record was written by an engine < 1.9.2 | "no attribution was collected" — *not* tampered |
| **refused before the check ran** | Trust-boundary (`TrustBoundary`) or boundary-strength (`Boundary`) refusal: the artifact was refused, so nothing was hashed or executed | "the check never ran" — the *summary* carries the reason |
| **artifact unreadable** | The check/authority existed but `sha256_file` could not read it (permissions, race) | "attribution was attempted and failed" — a weaker signal |

GAP-M-12's rule ("absent is not tampered") is still correct, but it is now too
coarse: a boundary refusal and a pre-attribution envelope render identically.
That matters precisely where the evidence is consumed — the enterprise Audit
Explorer renders nothing for a missing attribution block, so a **refusal** (the
most interesting case) is indistinguishable from an old record.

Restating the cause in prose does not scale: the summary text is
human-readable and localized, and a downstream consumer should not parse it.

## Decision

Add a structured, machine-readable reason to the finding.

```jsonc
// precondition_findings[] (rigorix-sdk $defs.preconditionFindingRef)
"attribution": "recorded" | "refused_before_check" | "artifact_unreadable"
```

- **Additive and optional.** Absent ⇒ unchanged meaning ("pre-attribution
  engine"), so existing envelopes stay valid and the "absent ≠ tampered" rule
  survives.
- **Present on new envelopes.** An engine that emits a finding always sets it,
  so absence unambiguously means "old engine" from the next release onward.
- `recorded` covers the normal case (including a *failed* check: attribution is
  present and the check ran). `refused_before_check` covers `TrustBoundary` and
  `Boundary`. `artifact_unreadable` covers a digest that could not be computed.
- `check_writable: null` (assessment unknown) remains a separate, orthogonal
  fact — it is a boundary-strength signal, not an attribution-absence signal.

Names are illustrative; the contract freeze decides the final wire name.

## Why this is scheduled with an SDK release

It changes the frozen canonical form of the envelope, so it must land as a
contract change **before** both servers implement it (D-011/D-012 sequencing;
this is the same order as #987 → SDK 0.3.0). Sequence:

1. `rigorix-sdk`: `$defs.preconditionFindingRef` gains the field; canonical
   (HMAC) ordering + fixtures updated; release (minor — additive).
2. `rigorix-oss` engine: populate it at the three sites
   (`service_impl.rs` finding construction; `runner.rs` refusal paths;
   `sha256_file` returning `None`).
3. `rigorix-enterprise`: ingest + serve it; the console renders it.
4. Enterprise conformance mirror (`core/tests/conformance/mod.rs`
   `CanonicalPreconditionFindingRef`) must be updated with the SDK change.
   Verified behaviour: this is a **test-only** mirror — production ingestion
   stores `precondition_findings` as raw `serde_json::Value`
   (`audit_ingestion/domain/value.rs`), so there is no field loss in
   production. A missed mirror update makes the conformance gate **fail
   loudly** (`HMAC verification failed (canonical form mismatch)`), which is the
   gate doing its job. Add the field with `skip_serializing_if = "Option::is_none"`
   and declare it **last**; that form is forward- *and* backward-compatible
   (verified 34/34 against both the pre-#994 and post-#994 SDK schemas), so it
   can land ahead of the SDK ref bump.

**Update (post-verification):** the mirror fix (item 4) has landed, and the SDK
contract change (item 1) is merged on `rigorix-sdk` main (`#42`) but **not yet
released** — the SDK version is still the already-published `0.3.0`, so it must
be bumped to `0.4.0` before any publish. Items 2 and 3 are held behind that
release for a demonstrated reason: the **published** `rigorix-verifier@0.3.0`
rejects an envelope carrying `attribution` (`signature mismatch`), so an engine
release emitting it before the SDK release would make every published verifier
reject valid envelopes.

## Interim behaviour (until this + the SDK ship)

A consumer that sees an attribution block missing must render the finding's
`summary` rather than showing nothing. The summary already distinguishes the
cases in prose (`argument 1 resolves inside the agent-writable workspace: …`
versus a `denied`/`failed` determination). Tracked for the console in the
enterprise repo (see the companion issue); ADR-017 §Evidence redaction records
the rule.

## Acceptance criteria

| # | Criterion |
|---|-----------|
| 1 | SDK: `preconditionFindingRef` carries the reason; canonical form + fixtures updated; conformance in both servers passes |
| 2 | Engine: a trust-boundary refusal records `refused_before_check` and **omits** the digests |
| 3 | Engine: a normal run (passed or failed) records `recorded` with the digests |
| 4 | Engine: an unreadable artifact records `artifact_unreadable` (not an absent block) |
| 5 | Old envelopes (no field) still verify against a new verifier — additive, no HMAC break |
| 6 | ADR-017 §Evidence redaction: the "Open" paragraph is replaced by the resolved state |
| 7 | Tests cover all four states plus the legacy-absent case |

## Files

- `engine/src/precondition/domain/finding.rs` (or the envelope finding model)
- `engine/src/precondition/application/service_impl.rs` (finding construction)
- `engine/src/precondition/infrastructure/runner.rs` (refusal paths, `sha256_file`)
- `engine/src/audit/domain/envelope.rs` (`PreconditionFindingRef`)
- `rigorix-sdk` `rust/rigorix-schemas` + `envelope.json` + fixtures
- `engine/.pi/architecture/decisions/ADR-017-consequence-gating.md` §Evidence redaction

## References

- ADR-017 §Evidence redaction (records this as open, scheduled with the next SDK release)
- GAP-M-12 (absent ≠ tampered) · #987 (attribution) · #986 (boundary) · #992 (check artifact)
- SDK 0.3.0 contract bump (the sequencing precedent)
