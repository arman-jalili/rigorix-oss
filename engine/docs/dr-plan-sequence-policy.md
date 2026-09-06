# Disaster Recovery Plan: sequence-policy Module

<!--
Canonical Reference: .pi/architecture/modules/sequence-policy.md
Last Updated: 2026-09-05
-->

## Scope

This DR plan covers the `sequence-policy` module — deterministic gates over
ordered step sequences: declarative rules (R1), plan-time evaluation (R2),
run-time prefix gate (R3), no-judgment determinism (R4), operator-controlled
rule authorship (R5), and envelope evidence (R6).

The module is **stateless between runs**:

| Tier | Artifact | Guarantee |
|------|----------|-----------|
| **Source of truth** | `.rigorix/sequence-policy.toml` (operator-authored, repo-controlled, agent-write-protected by R5) | Recreated from the repo on any environment |
| **Evidence (end-state)** | Rule decisions land in the signed audit envelope as `sequence_policy_findings[]` (plus `sequence_rule_matched` / `sequence_policy_denied` / `sequence_policy_config_error` events) | Tamper-evident when envelope signing is on; summaries redacted by default |
| **Operational (mid-run)** | A promoted node pauses inside the existing persisted `ExecutionState` (`AwaitingApproval`); resume re-reads rules fresh | Survives process restart via the standard hydrate/approve/resume flow — no module-owned state to restore |

## RTO/RPO Targets

| Metric | Target | Rationale |
|--------|--------|-----------|
| RTO (Recovery Time Objective) | < 1 minute | The module is stateless — recovery = ensure the operator rule file is present/valid and re-attach the service to the orchestrator/executor. A paused promoted run resumes via the existing persisted-state path |
| RPO (Recovery Point Objective) | 0 writes | The module writes nothing itself — rules are read-only per run; decisions are derived deterministically and re-derivable from (rule file + ordered plan) |

## Backup Strategy

**Rule file (`.rigorix/sequence-policy.toml`):**

- The rule file is part of the repository (same trust surface as
  `policy.toml` / `permissions.toml`) — the normal VCS backup cadence backs it
  up. Agent writes to `.rigorix/**` are denied by default permission config
  (R5), so the committed copy is the operator's.
- Keep the file **valid** as part of change control: a corrupt/over-cap file is
  **fail closed** at plan time (runs refused) — never silently degrade.

**Evidence (envelope):**

- Sequence-policy decisions are part of the audit envelope; back up envelopes
  on the audit module's schedule. Redacted summaries mean no parameter values
  need special handling in backups (SpanPrivacy default).
- Event-publish failures are warn-logged with the GAP-M-14 marker — never
  silent — so evidence gaps are visible in logs.

## Restore Procedure

1. **Restore the rule file** — `git checkout <commit> -- .rigorix/sequence-policy.toml`
   (or re-apply the operator-authored version). Validate it parses before
   relying on it: `rigorix_validate_plan` on a representative plan previews
   whether findings fire.
2. **No module state to restore** — the engine reads rules per run.
3. **Resume a paused promoted run** (if any were in flight):
   - Hydrate the persisted `ExecutionState` (cross-process resume path).
   - The promoted node is still `AwaitingApproval`; `approve_execution` +
     resume continues the run. Rules are re-read on resume — if the restored
     rule file differs, the gate re-evaluates with the restored rules
     (deterministic).

## Failover Plan

The module is a per-run in-process component with no leader/election, no
shared mutable state, and no external service. Failover = running the engine
elsewhere:

1. Provision the new environment with the same repository state
   (`.rigorix/sequence-policy.toml` included) and the same HMAC run key for
   envelope signing (evidence continuity).
2. In-flight runs are recoverable via persisted `ExecutionState`
   (hydrate → approve → resume); the R3 prefix gate re-evaluates the completed
   prefix from the hydrated graph on resume.
3. Determinism (R4): identical (rule file, ordered plan/prefix) input yields
   identical decisions — no cross-instance coordination is needed.

## RTO/RPO Verification (routine)

- On every proofing run, `check_sequence-policy_contracts.sh` (hardening stage
  36) verifies the module surface is implemented with no frozen stubs.
- `validate-architecture-readiness.sh` confirms runbook + DR plan + canonical
  refs + observability are present.
- Local recovery drill: delete `.rigorix/sequence-policy.toml` → confirm runs
  execute unchanged (fail-open-absent, AC#11); reintroduce a corrupt file →
  confirm plans are refused (fail-closed, AC#10); then restore the good file.
