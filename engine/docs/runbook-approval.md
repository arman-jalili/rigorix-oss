# Runbook: approval Module

<!--
Canonical Reference: .pi/architecture/modules/approval.md
Last Updated: 2026-09-02
-->

## Overview

The `approval` module binds human sign-off to the **exact resolved execution
intent** that will dispatch. It captures `(tool, intent, declared_scope)` at
approval time (R1), re-verifies it at the single dispatch choke point (R2),
records a signed single-use approval (R3), captures what the human was shown
(R4), and flags post-execution effects outside the declared scope (R5).

The defensive property this module establishes:

> **Shown = Dispatched = Executed (at dispatch level).** What the human was
> shown (`decision_context`) is derived from the same canonical
> `ExecutionIntent` that is hashed at approval time (`intent_hash`) and
> re-derived at dispatch time.

## Components

| Component | Type | Description |
|-----------|------|-------------|
| `ExecutionIntent` | Domain value object | Canonical `(tool, intent, declared_scope)`; sorted-key `canonical_bytes()` is the byte-level hash source |
| `IntentHash` | Domain digest | `HMAC-SHA256(run_key, canonical_bytes(intent))` — deterministic binding |
| `ApprovalRecord` | Domain aggregate | Single-use record: hash, payload, approver, authority, TTL, nonce, status, decision context |
| `DecisionContext` | Domain value object | What the human was shown; envelope-safe `summarize()` (SpanPrivacy redaction) |
| `ScopeViolation` | Domain evidence | Out-of-scope effects from the git-diff oracle (non-blocking, first-class) |
| `ApprovalService` / `ApprovalServiceImpl` | Application trait/impl | approve → verify_intent → consume lifecycle; migration invalidation |
| `ApprovalRepository` | Infra trait | Durable node-scoped records; `InMemory` + `FileBacked` implementations |
| `ApproveInput` / `ApproveOutput` | DTOs | Typed approval boundary (replaces the bare `step_names` surface) |

## Dependencies

| Dependency | Purpose | Failure behavior |
|------------|---------|------------------|
| State persistence (`ExecutionState`) | Durable approval records (operational tier) | Degrade to in-memory store + warning (same pattern as evaluation repository) |
| DAG engine `TaskNode` | Intent source (tool + intent) | Node without intent → approval cannot be captured; error surfaces to the approver |
| Identity claims | `approver_id` / `authority` / `token_claims_ref` | Degrades to captured fact; never fails the run |
| Audit envelope | `ApprovalRecorded` / `scope_violations[]` evidence | Records remain operational evidence; never over-claim tamper evidence when signing is off |

## Startup Sequence

1. **Configuration** — resolve the run key, the approval TTL, and the
   scope-declaration policy. The run key is the SAME key used for the envelope
   HMAC (ADR-011 §key).
2. **Repository** — open the durable approval store (state file). On hydrate,
   apply the **migration rule**: legacy `approved` sets (pre-binding, no
   records) are invalidated → re-approval required. No records are fabricated.
3. **Service wiring** — construct `ApprovalServiceImpl` with the repository, a
   `NodeIntentResolver` (sealed graph), and the `ScopeViolationSink` (audit
   envelope).
4. **Dispatch choke point** — `run_dispatch_loop` calls `verify_intent` before
   every dispatch of an approved node (covers both `execute_graph` and
   `resume_execution`).

## Graceful Shutdown

- **Pause mid-node**: a non-terminal interruption leaves the approval
  `Pending` — nothing is consumed. On resume in another process, the approval
  re-verifies against the re-derived intent (same graph → `Matched`).
- **Consume on terminal outcome**: `consume(node_id)` is called exactly once
  when a node reaches success, skipped, or exhausted failure (after ≥1
  dispatch). Failed attempts never consume.
- **Flush**: the `FileBackedApprovalRepository` writes atomically (temp file +
  rename) on every mutation — no data loss on interruption.

## Common Failure Modes and Recovery

| Failure mode | Symptom | Recovery |
|--------------|---------|----------|
| **Intent mismatch** (tampered state / upstream change) | `verify_intent` → `IntentMismatch`; node halts before dispatch; tool never called | Re-approval required. Do NOT auto-retry — a retry against a mutated intent is a replay loop (`IntentMismatch` is non-retriable). |
| **Expired approval** (TTL lapsed between approve and dispatch) | `verify_intent` → `Invalid(Expired)`; no dispatch | Re-approve. TTL is enforced at verification time, never at approval time. |
| **Replay of consumed approval** | `verify_intent` → `Invalid(Consumed)` (single-use) | Legitimate re-approval mints a fresh record with a new nonce. |
| **Cross-process resume against tampered persisted intent** | Process B re-derives a different hash than process A recorded | Halt; re-approval required. The persisted intent in the state file must be restored or re-approved. |
| **Legacy pre-binding approvals** | Persisted `approved` ids carry no records after upgrade | Invalidated on hydrate; verify returns `NotFound`; re-approval required. Never fabricate records. |
| **Scope oracle unavailable** (git missing / not a repo) | `ScopeVerificationUnavailable` (retriable) | Retry the oracle; if persistently unavailable, skip with an explicit `scope_verification: skipped` marker in the envelope — never silent. |
| **No approval record** for an approved-looking node | `verify_intent` → `NotFound` | Node was never approved (or legacy) — re-approval required. |
| **Repository write failure** | `ApprovalError::Internal` from save/load | Fail-closed: the node is not dispatched. Check state-file permissions/disk; the atomic rename protects the last good state. |

### Escalation

- Any `IntentMismatch` or `Invalid` verdict is **fail-closed** — the node is
  never dispatched. Escalate to a human approver for re-approval; do not
  bypass with an engine flag.
- Evidence of repeated mismatches on the same node (persistent tampering)
  should be treated as an incident — the persisted graph/state is suspect.
- Rollback: restore the pre-approval state file (see DR plan restore
  procedure) — the envelope signature will expose any drift on verify.

## Configuration Reference

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| Run key | bytes/secret | — | HMAC key for intent hashes — must equal the envelope HMAC key (ADR-011 §key) |
| Approval TTL | duration | deployment-defined | `expires_at = decided_at + ttl`; enforced at verification |
| Scope-declaration policy | config | per-run | Whether steps must declare scope at approval time (R1/R5) |
| Envelope signing | on/off | on | When off, records are operational evidence only — never claim tamper-evidence |

## Observability

- **Tracing**: `ApprovalServiceImpl` methods are `#[tracing::instrument]`-ed —
  approve, verify_intent (verdict field), consume, record_scope_violation.
- **Logging**: structured `tracing::info!/warn!` on approvals recorded,
  mismatches (warn), expiry, and consumption. Correlation is via the node_id /
  run context.
- **Metrics**: approval count, mismatch count, consume count, and TTL-expiry
  count are the key business signals (collected via the module's log/tracing
  surface; wire to Prometheus counters in the gateway layer).

## Related Documents

- DR plan: `docs/dr-plan-approval.md`
- Architecture: `.pi/architecture/modules/approval.md`
- Decision: ADR-011 (approval binding)
