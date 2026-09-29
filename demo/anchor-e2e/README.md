# Anchored-path E2E (ADR-016 Phase C, #912)

Proves the **wire**, not just fixtures: the OSS `HttpAnchorClient` talks to the
real enterprise anchor over HTTP, and the four outcomes hold.

| # | Outcome | Where |
|---|---------|-------|
| 1 | valid enterprise-signed slice verifies → deny-class run enforced | `engine/tests/anchored_wire_live.rs` |
| 2 | anchor unreachable → consequential run **fails closed** | idem |
| 3 | forged slice → **fails closed** (signature checked before matching) | idem |
| 4 | `local_unanchored` unchanged | `engine/tests/anchored_wire_e2e.rs` + `anchored_wire_live.rs` |

There are two layers:

- **`engine/tests/anchored_wire_e2e.rs`** — CI-able, no enterprise needed: a real
  HTTP server (wiremock) returns a genuinely Ed25519-signed slice on the
  enterprise's exact contract (`GET /v1/history?scope=&since=`, Bearer auth,
  bare signed slice). Covers valid / unreachable / erroring / forged /
  scope-mismatch / `local_unanchored` / envelope `anchor_head`.
- **`engine/tests/anchored_wire_live.rs`** — the true cross-process run against a
  live enterprise; **env-gated** (skips when `RIGORIX_E2E_ANCHOR_URL` is unset)
  so the normal suite stays green without an enterprise.

## Wire contract (pinned in both repos)

```
GET {anchor}/v1/history?scope=<producer_id>&since=<rfc3339>
Authorization: Bearer <rgx_... API key | JWT>
→ 200 { "scope", "since", "actions":[{ "node","principal","at",[effect_key] }],
        "head_hash", "sig" }        # bare signed HistorySlice (Ed25519, sig-null bytes)
```

Enterprise: `core/src/execution_api/interfaces/http/handlers.rs::history_router`
(#215). OSS:
`engine/src/audit/infrastructure/anchor.rs::HttpAnchorClient`; env
`RIGORIX_ANCHOR_URL` / `RIGORIX_ANCHOR_PUBLIC_KEY` / `RIGORIX_ANCHOR_TOKEN` /
`RIGORIX_ANCHOR_SCOPE`.

## Run it

Fast path (one command — disposable Postgres + prebuilt enterprise binary):

```bash
demo/anchor-e2e/run.sh          # exits 0 when all four outcomes pass
```

Compose path (builds the enterprise image):

```bash
docker compose -f demo/anchor-e2e/docker-compose.yml up -d --build postgres enterprise
# mark the services healthy, then run the host assertion with the env in docker-compose.yml
docker compose -f demo/anchor-e2e/docker-compose.yml down -v
```

`run.sh` env knobs: `ENTERPRISE_DIR`, `PG_PORT`, `ENT_PORT`, `ANCHOR_SCOPE`,
`KEEP=1`.

## DEV/TEST ONLY

`seed.sql` embeds an Argon2id hash for the fixture key
`rgx_dev_sk_e2e_secret_value` and the enterprise's dev anchor seed
(`[7u8;32]` → public key
`ea4a6c63e29c520abef5507b132ec5f9954776aebebe7b92421eea691446d22c`). Never use
these in production.
