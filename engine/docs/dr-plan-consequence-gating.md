# Disaster Recovery Plan: consequence-gating Module

<!--
Canonical Reference: .pi/architecture/modules/consequence-gating.md
ADR: .pi/architecture/decisions/ADR-017-consequence-gating.md
Last Updated: 2026-10-02
-->

## Scope

This DR plan covers the `consequence-gating` module (`engine/src/precondition/`)
— dispatch-time preconditions (R1), step-outcome gating (R2), and the
required-companion-step obligation (R3).

The module is **stateless between runs**:

| Tier | Artifact | Guarantee |
|------|----------|-----------|
| **Source of truth** | `.rigorix/preconditions.toml` (operator-authored, repo-controlled, agent-write-protected by ADR-013 R5) and the check programs it names | Recreated from the repo on any environment |
| **Evidence (end-state)** | `PreconditionChecked` events and the signed envelope `precondition_findings[]` (`precondition_id`, `step`, `outcome`, `exit_code`, `inputs_hash`, `checked_at`, `summary`) | Tamper-evident when envelope signing is on; no parameter values or stdout by default |
| **Operational (mid-run)** | The executor's persisted `ExecutionState`; preconditions are re-read fresh at each dispatch | Survives process restart via the standard hydrate/resume flow — no module-owned state |

## RTO/RPO Targets

| Metric | Target | Rationale |
|--------|--------|-----------|
| RTO (Recovery Time Objective) | < 1 minute | Stateless — recovery = ensure the config + check programs are present/valid and re-attach the gate to the executor |
| RPO (Recovery Point Objective) | 0 writes | The module writes nothing itself — the config and check programs are read-only per dispatch; verdicts are derived deterministically and re-derivable from (config + step + check process) |

## Backup Strategy

**Config (`.rigorix/preconditions.toml`):**

- Part of the repository (same trust surface as `policy.toml` /
  `permissions.toml`) — normal VCS backup cadence. Agent writes to `.rigorix/**`
  are denied by default (ADR-013 R5), so the committed copy is the operator's.
- Keep it **valid** as part of change control: a corrupt/over-cap file is
  **fail closed** for matching steps — never silently degrade.

**Check programs (`command[0]`):**

- The referenced check programs are operator-owned artifacts outside the
  agent-writable workspace. Back them up with the operator's standard process
  and keep their absolute paths stable across environments (the config names
  them literally).
- A check program that is missing at evaluation time yields an `error` refusal
  (`Spawn` / `unarmed`) — visible, never a pass.

**Evidence (envelope):**

- Precondition determinations are part of the audit envelope; back up envelopes
  on the audit module's schedule. The one-way `inputs_hash` means no parameter
  values need special handling in backups (SpanPrivacy).

## Restore Procedure

1. **Restore the config** — `git checkout <commit> -- .rigorix/preconditions.toml`
   (or re-apply the operator-authored version).
2. **Restore the check programs** — ensure every `command[0]` exists at its
   configured absolute path, is executable, and resolves outside the
   agent-writable workspace.
3. **No module state to restore** — the engine reads the config per dispatch.
4. **Resume / re-run** — resume a paused run via the standard persisted-state
   path; the gate re-evaluates each about-to-dispatch step with the restored
   config (deterministic).

## Failover Plan

The module is a per-run in-process component with no leader/election, no shared
mutable state, and no external service of its own (the operator's check process
is the only external dependency). Failover = running the engine elsewhere:

1. Provision the new environment with the same repository state
   (`.rigorix/preconditions.toml`) **and** the same check programs at the same
   paths. Preserve the HMAC run key for envelope-signing continuity.
2. In-flight runs recover via persisted `ExecutionState` (hydrate → resume);
   preconditions are re-read on resume.
3. Determinism (AC #9): identical (step + config + check process) input yields
   an identical verdict — no cross-instance coordination is needed.

## RTO/RPO Verification (routine)

- On every proofing run, `check_consequence-gating_contracts.sh` (hardening
  stage 37) verifies the module surface is implemented with no frozen stubs.
- `validate-architecture-readiness.sh` confirms the runbook + DR plan +
  canonical refs + observability are present.
- Local recovery drill: delete `.rigorix/preconditions.toml` → confirm runs
  execute unchanged (fail-open-absent); reintroduce a corrupt file → confirm
  matching steps are refused (fail-closed); then restore the good file.
- Trust-boundary drill: point `command[0]` at a script inside the workspace →
  confirm the step is refused with `error`; move it outside → confirm dispatch.
