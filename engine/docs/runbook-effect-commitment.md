# Runbook: Effect Commitment (`effect-commitment`)

**Status:** Implemented (ADR-019)
**Last updated:** 2026-10-23
**Canonical architecture:** `.pi/architecture/modules/effect-commitment.md`
**Decision:** `.pi/architecture/decisions/ADR-019-effect-commitment.md`

This runbook is for the operator on call when an execution contains an effect whose outcome is
unknown. The context answers one question — *was this effect committed?* — and it refuses to guess.

## What this context guarantees

| Level | Claim | Requires |
|---|---|---|
| L1 | The intent is recorded before the effect is sent; an `Ambiguous` effect blocks dependents and retry **in this process**. | `backend.kind = "in_memory"` |
| L2 | Records and fences survive process death and settle through a contested store. | `backend.kind = "http"` **and** a service implementing the frozen `/v1/effects` contract. **OSS ships the client, not the service**; enterprise ADR-011 specifies it. |
| L3 | The effect **committed**. | a `[receipt_reader]`, provider receipt/read-back evidence, and a provider that received the intent key |

Without a receipt reader the context can prove **not-settled**; it can never prove **settled**. Do
not describe a deployment as L3 unless its resolver actually presents provider evidence.

**Durability and settlement are two axes, not one ceiling (#1182).** L1/L2 describe how long the
record survives; L3 describes whether provider evidence can prove the effect committed. They
combine: an in-process ledger with a receipt reader is **L1 durability + L3 settlement**. That can
prove a commitment while the process lives, but a restart still loses the record and its fences, so
the receipt cannot be attached to anything afterward. `rigorix doctor` reports both axes and states
the restart cost; do not quote a single "L3" as if it implied durability.

## Who owns the `Dispatched` sweep (#1173)

The expiry predicate compares two values from two places:

```
record.started_at + 15min  <  Utc::now()
      ↑ written by the dispatching process      ↑ the sweeping process's clock
```

That comparison is only sound when both come from the same clock. The adapter reports the
capability (`can_sweep_dispatched`), and the sweep is gated on it — never on a `backend.kind`
string in each root:

| ceiling | sweeper | why |
|---|---|---|
| L1 (`in_memory`) | the composition root, at startup | record and clock are in one process; the root is the only possible sweeper |
| L2 (`http`) | the ledger service (enterprise ADR-011 `DispatchedSweeper`) | the store is shared; a client clock must not judge another process's `started_at` |
| L3 | the ledger service, same as its L2 store | provider evidence does not change who owns the records |

A client-side sweep against a shared store is unsound in both directions: a fast client clock marks
a live dispatch `Ambiguous` (a false double-spend signal); a slow one leaves orphans `Dispatched`
forever. The 15-minute lease only hides skew below fifteen minutes. `EffectCommitmentSetup::reconcile`
returns `0` without querying when the adapter cannot sweep.

## L2: the serving side is not in OSS (#1166)

The HTTP adapter is implemented and frozen, but nothing in this repository answers its six paths.
Every OSS deployment is therefore **L1** unless the operator supplies the enterprise ledger
service. That is the honest statement; "L2 available" without a service is the claim this section
exists to prevent.

If `backend.kind = "http"` is declared, startup probes the address and refuses to run when nothing
is listening, so the failure happens before the first effect rather than hours into a run. The
example declaration is [`docs/examples/effect-commitment.toml`](examples/effect-commitment.toml);
it is inert until copied to `.rigorix/effect-commitment.toml`.

## Startup sequence and dependencies

1. The composition root (`cli`, `mcp`, or `actions`) calls `EffectCommitmentSetup::from_env`.
2. If `.rigorix/effect-commitment.toml` is **absent**, no seam is armed: no adapter, no network, no
   shared state, and the previous dispatch path is unchanged.
3. If the declaration is present, it is parsed strictly. A malformed declaration, an unknown field,
   a missing `auth_token_env`, or an invalid URL is a **hard error at startup**. The run must be
   refused rather than silently downgraded.
4. The factory builds the ledger adapter and, when declared, the provider receipt reader. The
   composition root attaches `setup.dispatch_seam()` to the execution-engine factory.
5. At L1 the composition root runs `setup.reconcile()`. A `Dispatched` record older than the
   15-minute dispatch lease has no live owner (its process died), so it is moved to `Ambiguous`
   before any node can retry it. A sweep failure refuses the start; a fresh record is left alone
   because it may be in flight elsewhere. At L2 the call is a no-op in the client: the ledger
   service owns the sweep and its clock (see "Who owns the `Dispatched` sweep").

The only required dependency is the selected ledger adapter. The HTTP adapter additionally depends
on the ledger service; the receipt reader depends on the provider's read-back API.

## Configuration reference

```toml
# .rigorix/effect-commitment.toml

[backend]
kind = "in_memory"            # "in_memory" (L1) or "http" (L2)
# base_url = "https://commitment.example"   # required for "http"
# timeout_ms = 5000
# max_response_body_bytes = 65536
# auth_token_env = "EFFECT_LEDGER_TOKEN"     # env var, never the token itself

[receipt_reader]              # optional; without it the ceiling is L2
base_url = "https://payments.example"
provider_id = "stripe"
# timeout_ms = 5000
# max_response_body_bytes = 65536
# auth_token_env = "PAYMENTS_TOKEN"
```

| Variable / field | Meaning |
|---|---|
| `backend.kind` | `in_memory` = single process only; `http` = contested store |
| `backend.base_url` | Ledger service base URL; must be absolute `http(s)` |
| `backend.auth_token_env` | Name of the env var holding the bearer token; missing/empty is a startup error |
| `receipt_reader` | Optional provider read-back adapter; presence is the L3 claim |
| `timeout_ms` | Per-request deadline; must be > 0 |
| `max_response_body_bytes` | Bounded response body; must be > 0 |

## L3: what the provider must do (#1165)

The receipt reader looks a receipt up by the intent's idempotency key. That only works if the
provider was given the key when the effect was sent:

- **Process providers** invoked through `run_command` receive
  `RIGORIX_EFFECT_IDEMPOTENCY_KEY=<intent-key>` in their environment. This is the implemented
  transmission.
- **HTTP providers** would receive `Idempotency-Key: <intent-key>`, but the engine has no HTTP
  tool: every tool sink is a process. The header is **reserved** for a future HTTP adapter (#1174),
  not a mechanism an integration can rely on today.

The provider MUST dedupe on that key and MUST expose it on the receipt (the reader queries
`GET /v1/receipts?idempotency_key=<key>`). A provider that stores the key but does not dedupe leaves
L3 out of reach. The key is not a secret, but it must never be logged together with request or
response bodies. Without a provider that honours this contract, the honest ceiling is L2.

## Operating an unsettled effect

An `Ambiguous` effect blocks its dependents and any retry **by design**. Do not wait for a timeout
to make it disappear: a guessed outcome is how the window is closed incorrectly.

1. **Find it.** Query the ambiguous queue:
   `GET /v1/effects/ambiguous` (optionally `?dag_id=<uuid>`), or the service's
   `query(EffectQuery::ambiguous(...))`. The result shows the record id, DAG, node, intent digests,
   and the reason the outcome is unknown.
2. **Get evidence.** Read the provider receipt/read-back. The receipt must identify the effect and
   prove committed or not-committed. A local result is not provider evidence.
3. **Resolve it.** `POST /v1/effects/{id}/resolve` with `ObservedOutcome` and a `ResolverIdentity`
   whose kind is `provider` or `external_operator`. The service records who resolved and on what
   evidence digest.
4. **Never resolve with** `dispatching_process` or `agent`. Those are rejected by policy (F4): they
   provably cannot know.
5. **Resume the run.** Resolution settles the record; dependents that were fenced are released by
   the operator's normal resume/approval path. A blocked run does not auto-resume.

## Common failure modes and recovery

| Symptom | Meaning | Recovery |
|---|---|---|
| Dispatch refused with `effect commitment begin failed` | The write-ahead record could not be written. | Treat as fail-closed. Check the ledger service/network, then retry the run. Do **not** bypass the seam. |
| `Ambiguous` effect; run does not progress | A response was lost, a deadline passed, the process died, or a cancellation happened in flight. | Follow *Operating an unsettled effect*. |
| A record is still `Dispatched` after a crash (**L2 only** — at L1 the record is gone, not stale) | The startup sweep has not run, or the record is still inside its 15-minute lease (it may be in flight elsewhere). | Restart the composition root so the sweep runs, or wait for the lease; then follow *Operating an unsettled effect*. Never resolve a `Dispatched` record by hand as `NotCommitted`. |
| `retry refused` on a node | F2: the intent is `Ambiguous` (unknown), or it is already `Committed` (the work is done). | Resolve the ambiguous record; for a committed effect, do not re-send. A `NotCommitted` effect is retryable as a new attempt under the same key. |
| Dependents never run | F1: an upstream effect is still ambiguous. | Resolve the upstream record; then resume. |
| `provider receipt status 'pending'` / `Unavailable` | The provider has not settled; the context refuses to guess. | Retry the read-back later. The effect stays ambiguous. |
| `effect-commitment config invalid` at startup | Malformed/incomplete declaration or missing credential env var. | Fix the TOML/env and restart. The run must not start with a guessed config. |
| `effect-commitment ledger unavailable` | The L2 store is unreachable. | Fail-closed. Restore the store; see the DR plan. |

## Graceful shutdown

1. Stop accepting new work at the orchestrator/host boundary.
2. Let in-flight nodes drain where possible. A node cancelled in flight is marked `Ambiguous`, not
   failed: the effect may have happened.
3. Do not discard the ledger. For L2, the store retains the records; for L1, process death loses
   them — that is the L1 ceiling, not a bug.
4. Before closing an execution, query the ambiguous queue. A run with an unsettled effect must stay
   open until an authorised resolver settles it.

## Observability

- **Tracing.** The service emits structured events on `begin` (debug), `settle_definite` (debug),
  `mark_ambiguous` (warn), `resolve` (info), and `query` (debug). Fields include `record`, `dag`,
  `node`, `state`, and `resolver`. Credentials are never logged.
- **Audit/evidence.** `ResolveEffectOutput` carries the `ObservedOutcome` and the `ResolverIdentity`;
  the resolved record is the evidence for the audit chain.
- **Metrics.** The engine emits logs/events; the external L2 store owns request/latency/error
  metrics. Monitor the ambiguous queue size (query the ambiguous surface) and the ledger's
  availability. There is no engine-local counter for commitments — the store is the authority.
- **Health.** The host `/health` checks the process. The seam's dependency check is the startup
  `EffectCommitmentSetup::from_env` (fail-closed) and the ledger adapter's per-request deadline.
  Alert on repeated `Unavailable` responses from the ledger.

## Incident, escalation, and rollback

- **Incident:** an execution is blocked on `Ambiguous`, or the ledger is unavailable.
- **Escalation:** page the service owner for the ledger/provider, not the agent. The agent cannot
  resolve the effect (F4). Escalate to the operator authorised to present provider evidence.
- **Rollback:** removing `.rigorix/effect-commitment.toml` returns the deployment to the previous
  dispatch path on the next start. Do this only with the ledger owner's agreement: existing records
  remain in the L2 store, and unresolved effects are still owed a decision. Never delete an
  ambiguous record to unblock a run.
