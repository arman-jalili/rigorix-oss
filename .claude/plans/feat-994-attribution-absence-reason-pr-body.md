# feat: #994 — structured attribution reason on precondition findings

Implements **#994** (`ISSUE-ATTRIBUTION-ABSENCE-REASON`, epic `consequence-gating`).

## Problem

Absence of the ADR-017 attribution digests was overloaded — it meant three
different things, and a consumer could not tell a **boundary refusal** (the
interesting case) from an old record:

| Cause | What happened |
|---|---|
| pre-attribution engine | record written by an engine < 1.9.2 |
| refused before the check ran | `TrustBoundary` / `Boundary` refusal — nothing hashed or executed |
| artifact unreadable | a digest could not be computed (permissions / race) |

## Change

A structured, machine-readable reason on the finding:

```jsonc
// precondition_findings[]
"attribution": "recorded" | "refused_before_check" | "artifact_unreadable"
```

- **Additive and optional.** Absent ⇒ unchanged meaning (pre-attribution engine),
  so existing envelopes stay valid and "absent ≠ tampered" survives.
- **Present on new envelopes.** An engine that emits a finding always sets it.
- `recorded` — the check ran (passed or failed) and attribution was collected.
- `refused_before_check` — trust-boundary / boundary-strength / `require_params`
  / timeout / spawn refusal.
- `artifact_unreadable` — a digest could not be read; any digest that *could* be
  computed is still present.

## Contract sequencing (D-011/D-012)

The change lands in `rigorix-sdk` **first** (sibling PR: `$defs.preconditionFindingRef`
+ canonical fixtures + all five verifiers). This PR populates it in the engine.
A minor SDK release (additive) precedes the enterprise ingest/console work; the
enterprise conformance mirror must move in lockstep (serde drops unknown fields).

## Files

- `engine/src/precondition/domain/finding.rs` — `AttributionReason`; optional
  `attribution` on `PreconditionFinding` / `PreconditionChecked` (serialized last).
- `engine/src/precondition/application/service_impl.rs` — set at all three sites.
- `engine/src/precondition/application/surfaces.rs` — surfaced by
  `rigorix_validate_plan`.
- `engine/src/event_system/domain/event.rs` + `execution_engine/.../dispatch.rs` —
  carry it through the event payload.
- `engine/src/audit/{domain/envelope,application/envelope_factory_impl}.rs` —
  `PreconditionFindingRef`.
- `mcp/src/audit_tools/**` — mirror on the MCP read surface.
- `engine/.pi/architecture/decisions/ADR-017-consequence-gating.md` — §Evidence
  redaction resolved.

## Acceptance criteria

| # | Criterion | Status |
|---|-----------|--------|
| 1 | SDK carries the reason; canonical + fixtures updated; conformance in both servers | ✅ contract-first (sibling PR) |
| 2 | Trust-boundary refusal → `refused_before_check`, **omits** digests | ✅ |
| 3 | Normal run (passed or failed) → `recorded` with digests | ✅ |
| 4 | Unreadable artifact → `artifact_unreadable` | ✅ |
| 5 | Old envelopes still verify against a new verifier (additive, no HMAC break) | ✅ |
| 6 | ADR-017 §Evidence redaction replaced with the resolved state | ✅ |
| 7 | Tests cover all states + legacy-absent | ✅ |

## Validation

```
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test -p rigorix-engine --features conformance --test conformance
bash .pi/scripts/validate-ci.sh validate-tests.sh validate-security.sh validate-operations.sh
```

All green. Engine↔SDK conformance passes against the sibling SDK branch.

Refs #994
