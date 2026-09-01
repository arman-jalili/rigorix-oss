# fix(audit): full-envelope HMAC + lifecycle events + evidence marker

**Batch 3** of the gap-ledger implementation backlog (`feature/audit-evidence`).
Closes **#723** (GAP-A-06), **#755** (GAP-M-12), **#734** (GAP-A-17), **#752** (GAP-L-09).

## #723 — HMAC now covers the FULL canonical envelope

Engine `compute_signature` signed only 7 scalar fields — event contents, `file_paths`, `scoring_results`, `identity` and git metadata were unsigned, so tampering with any evidence field went undetected. MCP `compute_hmac` used a different, equally partial field set.

- Engine: sign the **full envelope as sorted-key canonical JSON** (serde_json `Map` is BTreeMap-backed without `preserve_order` → deterministic); the `signature` field is excluded so sign and verify hash identical bytes
- MCP: mirrored the same full-canonical-serialization approach
- **Tests:** tampering `events[0]`, `file_paths`, or `scoring_results` breaks verification; an untampered envelope still verifies

## #755 — explicit `evidence_degraded` marker

`AuditEnvelope.evidence_degraded` (serde-defaulted) is set true when signing is not requested, so an unsigned envelope is explicit degraded evidence. Sign-without-key still hard-fails (the M-12 "fail" branch). Test covers both states. This is the hook the approval epic needs: approval-bearing runs must request signing or carry the marker.

## #734 — envelope lifecycle events are emitted

`ExecutionEvent` gains `AuditEnvelopeDelivered` / `AuditEnvelopeQueued` / `AuditEnvelopeDropped` / `CircuitBreakerStateChanged` (execution_id + timestamp, consistent with the enum contract). `AuditSenderImpl` accepts an optional event bus (`with_event_bus`, default `None` = zero behavior change) and emits the lifecycle events from `send` / `deliver_with_retry`. All exhaustive matches updated (event_system, orchestrator, repository, CLI TUI bridge). **Tests:** queued-then-dropped and breaker-open emission via a real bus.

## #752 — git provenance verified + regression test

**Verified:** the orchestrator's `detect_git_info` (orchestrator_impl.rs:814-825) populates the record context → `BuildEnvelopeInput` → envelope. The `git_commit: None` sites in queue/sender/repository are **test fixtures**, not production paths. Added a factory regression test asserting git fields carry through.

## Verification
- 1894 engine lib tests (+5 new), 2500 workspace tests total
- `clippy -D warnings` clean, `fmt --check` clean, workspace builds
- mcp audit_tools tests pass (19)

**Note:** the full-envelope HMAC changes the signature bytes vs the old scalar-subset format. Signatures are computed and verified in-process at build/verify time; no persisted signature crosses this format boundary.
