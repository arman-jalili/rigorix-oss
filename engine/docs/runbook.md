# Rigorix Engine Runbook Index

**Last updated:** 2026-10-21

This is the entry point for operating the engine. Each bounded context has its own runbook with the
failure modes, recovery steps, and claim ceiling specific to it.

## Module runbooks

| Context | Runbook | DR plan |
|---|---|---|
| Effect commitment (ADR-019) | [`runbook-effect-commitment.md`](runbook-effect-commitment.md) | [`dr-plan-effect-commitment.md`](dr-plan-effect-commitment.md) |
| Capacity ledger | [`runbook-capacity-ledger.md`](runbook-capacity-ledger.md) | [`dr-plan-capacity-ledger.md`](dr-plan-capacity-ledger.md) |
| Precondition gate | [`runbook-precondition.md`](runbook-precondition.md) | [`dr-plan-precondition.md`](dr-plan-precondition.md) |
| Approval binding | [`runbook-approval.md`](runbook-approval.md) | [`dr-plan-approval.md`](dr-plan-approval.md) |
| Audit / evidence | [`runbook-audit.md`](runbook-audit.md) | [`dr-plan-audit.md`](dr-plan-audit.md) |
| Identity | [`runbook-identity.md`](runbook-identity.md) | [`dr-plan-identity.md`](dr-plan-identity.md) |

The per-module docs are the authoritative recovery procedures. Start with the module whose failure
you are investigating; the sections below are the cross-cutting incident, escalation, rollback,
recovery, and shutdown rules.

## Incident response

1. **Classify the failure.** Is the run blocked (approval, precondition, capacity, an ambiguous
   effect), or is a dependency down (ledger, provider, model, filesystem)?
2. **Prefer the fail-closed path.** Refused dispatch is a correct outcome. Do not bypass a gate or
   delete an unsettled record to make a run progress.
3. **Preserve evidence.** Audit envelopes, precondition findings, capacity holds, and commitment
   records are the explanation of what happened. Do not truncate or rewrite them.
4. **Open an incident** for any run that cannot be closed without operator resolution, or any
   dependency outage that refuses dispatch.
5. **Escalate** to the owning service/team: ledger owner for L2 effects, provider owner for L3
   evidence, platform on-call for the engine process itself. The agent is never the resolver of an
   ambiguous effect.

## Recovery

- Follow the module runbook's recovery table first.
- After a dependency recovers, re-run the blocked execution through the normal resume/approval path;
  do not replay a settled effect.
- Verify the claim ceiling before declaring recovery: an in-process (L1) ledger cannot prove
  survival across the restart, and without a receipt reader the system can prove **not-settled**
  only.

## Rollback

- Configuration rollback is opt-in: remove or disable the module declaration
  (`.rigorix/effect-commitment.toml`, `.rigorix/capacity.toml`, `.rigorix/preconditions.toml`, …) and
  restart. The previous code path is restored on the next start.
- Do not roll back by deleting durable records. Existing records remain the evidence of what was
  attempted; unresolved effects are still owed a decision.
- If a bad build is the cause, roll back the binary, keep the data, and reconcile before reopening
  runs.

## Graceful shutdown

1. Stop accepting new executions at the host boundary (CLI/MCP/Actions).
2. Allow in-flight nodes to drain; a cancellation in flight is recorded as an ambiguous effect, not
   a failure.
3. Flush/persist state as the deployment requires: state persistence, audit, and the L2 commitment
   store.
4. Before the final exit, check for blocked runs (approval-paused, capacity-denied, ambiguous
   effects) and record them for the next operator. A run with an unsettled effect stays open.

## Health and observability

- The host exposes process health; each adapter enforces its own deadline and fails closed.
- Structured tracing is emitted with module/record identifiers. Credentials are never logged.
- Monitor the failure surfaces named in the module runbooks (ambiguous effect count, ledger
  availability, provider read-back latency, capacity exhaustion).
