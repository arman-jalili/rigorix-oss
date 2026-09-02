# Disaster Recovery Plan: approval Module

<!--
Canonical Reference: .pi/architecture/modules/approval.md
Last Updated: 2026-09-02
-->

## Scope

This DR plan covers the `approval` module — the consequence-bound human
sign-off layer: intent capture + hashing (R1), pre-dispatch verification (R2),
single-use signed approval records (R3), decision context (R4), and
effect-scope verification (R5).

The module has two persistence tiers:

| Tier | Artifact | Guarantee |
|------|----------|-----------|
| **Evidence (end-state)** | Approval records are content of the signed audit envelope (`approval_events[]`) | Tamper-evident when envelope signing is on |
| **Operational (mid-run)** | Records appended to the persisted `ExecutionState` file (`approval_records`) via `ApprovalRepository` | Survives cross-process resume; atomic writes |

## RTO/RPO Targets

| Metric | Target | Rationale |
|--------|--------|-----------|
| RTO (Recovery Time Objective) | < 1 minute | The module is stateless in memory — recovery = reopen the state file and re-wire the service; approvals re-verify on first dispatch |
| RPO (Recovery Point Objective) | ≤ 1 write | `FileBackedApprovalRepository` writes atomically (temp + rename) on every mutation — at most the in-flight write is lost on hard crash |

## Backup Strategy

**Operational tier (mid-run state file):**

- The approval store is a small JSON file (node-scoped records; approvals are
  rare). Back it up with the run's persisted state on the same schedule as
  `ExecutionState` (the module has no separate backup cadence).
- Writes are atomic (temp file + rename) — a crash never corrupts the last
  committed record.

**Evidence tier (signed envelope):**

- The authoritative, tamper-evident copy of every approval decision lives in
  the end-of-run signed audit envelope (`approval_events[]`). Backup of the
  envelope is the module's durable audit record — retain per the audit
  retention policy.

**No backups required for:** the in-memory caches or derived intents — every
approval record embeds its own canonical `intent_payload` and `intent_hash`,
so verification never depends on external state.

## Restore Procedure

1. **Restore the state file** from the last good backup (or from the last
   atomic write — the file is never left half-written).
2. **Open the repository** at the restored path. Hydration applies the
   migration rule: legacy `approved` ids without records are invalidated →
   re-approval required (never fabricate records).
3. **Resume the run** (process B). Each approved node re-verifies at the
   dispatch choke point:
   - Identical graph → `Matched` → execution continues.
   - Tampered/missing intent → `Mismatched` / `NotFound` → halt, re-approval
     required. This is the module's integrity guarantee — restoring a stale
     graph can never authorize a stale approval against different bytes.
4. **Validate the envelope** at run end — `approval_events[]` must match the
   restored operational records.

## Failover Plan

- **Cross-process resume is the failover path**: process A pauses mid-node
  (approval stays `Pending`), process B opens the same store and continues.
  No leader election or shared lock is required — single-node, single-writer
  per run.
- **Verification is the safety net**: any divergence between what process B
  re-derives and what process A recorded halts dispatch. Fail-closed by
  design — an approval-service error never silently dispatches.

## RTO/RPO-Informed Runbooks

| Scenario | Procedure | Meets |
|----------|-----------|-------|
| Crash after approve, before dispatch | Reopen store → resume → verify (TTL enforced) → dispatch or re-approve | RTO < 1 min, RPO ≤ 1 write |
| State file corrupted | Restore backup → hydrate (migration rule) → resume | RPO = backup cadence |
| Cross-process handoff with tampered state | Resume halts on `Mismatched` → restore or re-approve | Integrity > availability (documented) |
| Envelope signing disabled | Records are operational evidence only — restore from state file; docs/UI must not claim tamper evidence | Trust model note (ADR-011 §key) |

## Related Documents

- Runbook: `docs/runbook-approval.md`
- Architecture: `.pi/architecture/modules/approval.md` (Durability + Migration Rule)
- Decision: ADR-011 (approval binding)
