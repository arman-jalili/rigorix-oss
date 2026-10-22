# DR Plan: Effect Commitment (`effect-commitment`)

**Status:** Implemented (ADR-019)
**Last updated:** 2026-10-23
**Companion runbook:** `docs/runbook-effect-commitment.md`

## Scope

The commitment record is the evidence that an effect was attempted and, when known, what happened.
Losing it is not a cache miss: it reopens the window where the system cannot distinguish
"never happened" from "committed".

| Deployment | State owner | What DR must protect |
|---|---|---|
| L1 (`in_memory`) | the engine process | Nothing durable. Process death loses the records; this is the documented L1 ceiling. |
| L2 (`http`) | the contested ledger store | The records and their transitions. This is the DR scope. |
| L3 (settlement) | the provider receipt system + whichever ledger is configured | Provider evidence. The provider is the authority; the store is the index. L3 is orthogonal to durability: at L1 the receipt can prove a commitment during the process, but a restart loses the record it would resolve. |

## Backup strategy

- **L2 store:** use the store's native backup/PITR. Back up the commitment records and their
  transitions together — a record without its resolver/outcome is not evidence.
- **Frequency:** at least every 5 minutes (or the store's continuous WAL/PITR). Keep the backup in a
  separate failure domain from the primary.
- **Provider receipts:** retain them under the provider's own retention policy. The L2 backup cannot
  recreate a receipt that the provider no longer has.
- **Configuration:** version-control `.rigorix/effect-commitment.toml` in the repository. Credentials
  live in the secret manager, named by `auth_token_env`; never back up the token in the TOML.
- **L1:** no backup is possible. Do not run L1 where the commitment guarantee is claimed to survive
  a restart. Document the ceiling in the deployment's own runbook.

## Restore procedure

1. Restore the L2 store into an isolated environment and verify the record count and the ambiguous
   queue against the last known-good snapshot.
2. Verify a sample of terminal records has both an `ObservedOutcome` and (for resolved records) a
   `ResolverIdentity`. A terminal record without evidence is corrupt; do not serve it as settled.
3. Point the `backend.base_url` at the restored store (or the promoted replica) and restart the
   composition root. `EffectCommitmentSetup::from_env` must succeed; a malformed config refuses to
   start. The **service** then reconciles: at L2 it owns the store and the clock, and moves
   `Dispatched` records older than the 15-minute dispatch lease to `Ambiguous`. The client's
   `EffectCommitmentSetup::reconcile` is a no-op for a shared ledger, because a client clock must
   not judge another process's `started_at` (#1173). An L1 root still sweeps at startup — it owns
   the only clock.
4. Re-query the ambiguous queue — it now includes both effects that lost their response and effects
   whose owner died while still `Dispatched`. Every unsettled effect remains fenced; resolve each
   with provider evidence before closing its run. Do **not** bulk-mark them `NotCommitted` to clear
   the queue.
5. Reconcile the audit chain: the resolved records are the evidence for the interval.

### Reconciler per ceiling

| ceiling | who reconciles `Dispatched` after a crash | clock |
|---|---|---|
| L1 | the composition root, at startup | the root's clock (same process that wrote the record) |
| L2 | the ledger service (`DispatchedSweeper`, enterprise ADR-011) | the service's clock (same store that owns the records) |
| L3 | the ledger service, as L2 | the service's clock |

Do not restore an L2 store and then sweep it from a client: the records were written by other
processes. If the service does not implement the sweeper, orphans stay `Dispatched` and the
ambiguous queue is incomplete — escalate to the ledger owner rather than shrinking the lease.

## Failover plan

- **Store failover:** promote the replica and update `backend.base_url`/DNS. The HTTP adapter fails
  closed on unreachable/timeout/contended responses; in-flight dispatch is refused, never silently
  allowed.
- **Provider failover:** the receipt reader is per-provider (`receipt_reader.base_url` +
  `provider_id`). Point it at the provider's secondary read-back surface; the produced evidence still
  names its source. If no read-back is available, the context can only prove not-settled.
- **Credential rotation:** update the environment variable named by `auth_token_env` and restart. The
  token is never persisted in the TOML or logged.
- **Regional loss:** restore the L2 store in the recovery region, then restart. The provider remains
  the L3 authority.

## RTO / RPO targets

| Scenario | RTO | RPO | Notes |
|---|---|---|---|
| L2 store failover | 1 hour | 5 minutes | Depends on the store's replication/PITR; the engine has no local state. |
| L2 store restore from backup | 4 hours | 5 minutes | Reconcile the ambiguous queue before reopening runs. |
| Provider read-back outage | n/a | n/a | No data loss; the context refuses to guess until the provider returns. |
| L1 process restart | 0 | **all records** | No durability claim; the previous run's records are gone by design. |

These targets are for the commitment context only. The host's overall RTO/RPO may be tighter or
looser; align them with the ledger owner.

## Claim ceiling and drills

- L1: record + fences in-process only.
- L2: survives process death; settles through a contested store.
- L3: "committed" requires provider evidence. A restore drill that only restores the store proves L2,
  not L3.

**Quarterly drill:** restore a store snapshot into isolation, run the ambiguous query, resolve one
record with a sandbox provider receipt, and verify the fence lifts and the audit chain accepts the
resolved record. Record the achieved RTO/RPO against the targets above.
