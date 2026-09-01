# Batch Plan — feature/audit-evidence

**Source:** `.claude/plans/issue-groups.md` batch 3
**Issues:** #723 GAP-A-06, #755 GAP-M-12, #734 GAP-A-17, #752 GAP-L-09
**Files:** `engine/src/audit/**`, `engine/src/event_system/domain/event.rs`, `mcp/src/audit_tools/infrastructure/in_memory_audit_service.rs`, tests

## Implementation order

| # | Issue | Change | Risk |
|---|-------|--------|------|
| 1 | #723 A-06 HMAC full envelope | Engine `compute_signature` → full canonical sorted-key serialization (signature excluded); MCP `compute_hmac` → same approach; tamper tests (events, file_paths, scoring_results) | MEDIUM (signature format change — signatures not persisted across the format change; verification same-process) |
| 2 | #755 M-12 evidence-degraded marker | `AuditEnvelope.evidence_degraded` (serde default); set true when `sign: false`; keep hard-fail on sign-without-key; test | LOW |
| 3 | #734 A-17 AuditEvent emission | Add 4 lifecycle variants to `ExecutionEvent`; `AuditSenderImpl.event_bus` (optional, `with_event_bus`); emit Delivered/Queued/Dropped/CircuitBreakerStateChanged; mock-bus tests | LOW (opt-in, default None) |
| 4 | #752 L-09 git fields | VERIFIED: orchestrator `detect_git_info` populates context → envelope; the `None` sites are test fixtures. Add factory regression test asserting git fields carry through | LOW |

## Validation

```bash
cargo test -p rigorix-engine --lib audit event_system
cargo clippy -p rigorix-engine -- -D warnings
cargo fmt --check
```

## Commits

1. `fix(audit): HMAC covers the full canonical envelope (engine + mcp) (#723)`
2. `feat(audit): evidence_degraded marker for unsigned envelopes (#755)`
3. `feat(audit): emit envelope lifecycle events (delivered/queued/dropped/breaker) (#734)`
4. `test(audit): git_commit/git_branch carry into the envelope (#752)`
