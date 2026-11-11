# Licensing & Enterprise Boundary — Deep Findings

Status: evidence-based analysis of `rigorix-oss` (workspace: engine, cli, actions, mcp, server).
OSS license: `MIT OR Apache-2.0` (root + every crate). Enterprise plane (the enterprise repository)
is a separate workspace/repo and is proprietary.

## 1. What actually ships in rigorix-oss

| Crate | Role | Network by default? |
|---|---|---|
| `engine` | audit, approval, capacity, execution orchestration, anchor client | No (anchor unarmed unless env set) |
| `cli` | orchestrator; sets `approval_repo_path = repo_root` | No |
| `actions` | GitHub Actions composite | No |
| `mcp` | MCP gateway: 15 OSS tools + optional enterprise proxy | No (proxy disabled unless env set) |
| `server` | HTTP server exposing `rigorix.system.version` + anchor status | No |

All crates declare `license = "MIT OR Apache-2.0"`. No crate depends on an enterprise crate.

## 2. The only outward seams (enterprise connection points)

### A. History anchor — engine (`engine/src/audit/infrastructure/anchor.rs`)
Env: `RIGORIX_ANCHOR_URL` (+ `RIGORIX_ANCHOR_PUBLIC_KEY`), optional `RIGORIX_ANCHOR_SCOPE`, `RIGORIX_ANCHOR_TOKEN`.
- Anchored **iff both** URL and public key are set and non-empty; otherwise `local_unanchored`.
- HTTP client does `GET {base}/v1/history`, optional `Authorization: Bearer <token>`.
- Slice is **Ed25519-verified before any action**; scope checked; unreachable/unsigned/forged ⇒ fail-closed error.
- This is the only place OSS reads from the enterprise plane, and it is opt-in.

### B. Enterprise proxy — MCP gateway (`mcp/src/enterprise_proxy`, `mcp/src/host/mod.rs`)
Env: `ENTERPRISE_API_URL`, `ENTERPRISE_API_KEY`, `ENTERPRISE_TIMEOUT_SECS`, `ENTERPRISE_TLS_VERIFY`, `ENTERPRISE_SCHEMA_TTL_SECS`.
- `try_init_enterprise_proxy()` returns `None` when URL/key absent ⇒ proxy disabled, 15 OSS tools only.
- When enabled, proxies `rigorix_enterprise_*` tool calls over **HTTPS JSON-RPC**; tools discovered dynamically at init.
- `ProxyConfig` **requires HTTPS** (`api_url.starts_with("https://")`) and non-empty key.
- API key stored as `Secret<String>`: redacted in Debug/Display/Serialize; `expose()` deliberate.

### C. Local-only state (not enterprise)
`RIGORIX_HMAC_KEY`, `RIGORIX_REPO_ROOT`, `RIGORIX_SERVER_BIND`, `approval_repo_path`,
approval store, consumption ledger, effect commitment, capacity/budget — all local.
`approval_repo_path` is used only by the OSS in-process git-oracle diff check
(`engine/src/execution_engine/application/service_impl/dispatch.rs`); never by an
enterprise write path.

## 3. Findings

| # | Finding | Severity | Status |
|---|---|---|---|
| F1 | Two independent env naming schemes (`RIGORIX_*` anchor vs `ENTERPRISE_API_*` proxy) with no documented mapping. | Low (docs) | Document |
| F2 | Offline verifier refused only `RIGORIX_ANCHOR_URL`/`RIGORIX_ENTERPRISE_URL`/`RIGORIX_ENTERPRISE_TOKEN`; it did not refuse `ENTERPRISE_API_URL`/`ENTERPRISE_API_KEY`. | Medium | Fixed in verifier |
| F3 | `ENTERPRISE_TLS_VERIFY` can be set to `false`/`0`/`no`, disabling TLS verification on the enterprise connection. | Medium (hardening) | Recommend disallowing in prod |
| F4 | The enterprise proxy module ships inside `rigorix-oss` under the OSS license. Thin HTTP adapter, no enterprise IP; schemas fetched at runtime. | By design | Acceptable |
| F5 | Enterprise tool schemas cached in-memory only (`SchemaCache`); no disk persistence. | Low | Acceptable |
| F6 | Anchor bearer token (`RIGORIX_ANCHOR_TOKEN`) sent via `bearer_auth`, not logged. | Good | Keep |
| F7 | No code-level license enforcement; boundary is operational (env, keys, HTTPS, separate repos). | By design | Standard SaaS |
| F8 | Enterprise tool discovery is forward-compatible: new tools need no OSS change. | Good | Keep |
| F9 | MCP exposes exactly 15 OSS tools when unconfigured; enterprise tools only when configured (verified by e2e test). | Good | Keep |
| F10 | Anchor mode reported via `rigorix.system.version` (`local_unanchored` vs `anchored`). | Good | Keep |
## 4. How to protect the enterprise

1. **Keep enterprise code proprietary and separate.** Separate repo/workspace/CI; never vendor or include OSS source into enterprise artifacts.
2. **Auth at the edge.** Enterprise `/v1/history` and tool endpoints require an `rgx_` API key or JWT; OSS only presents credentials via env. Rotate keys; never embed in OSS defaults.
3. **Rate limit + audit at the enterprise edge.** OSS client is untrusted; all enforcement happens server-side.
4. **TLS only.** `ProxyConfig` already rejects non-HTTPS; consider removing the `tls_verify=false` escape hatch or gating it behind an explicit dev flag.
5. **Do not publish enterprise tool schemas in OSS.** Fetched at runtime, cached in memory only.
6. **Fail closed.** Anchor verification (`verify_slice`) and proxy errors never cascade to OSS tools; clear diagnostics returned.
7. **Version the wire contract.** Frozen `EnterpriseProxy` trait + `SchemaCache`; forward-compatible discovery lets enterprise evolve without OSS releases.

## 5. Licensing model

| Component | License | Rationale |
|---|---|---|
| `rigorix-oss` (engine, cli, actions, mcp, server) | `MIT OR Apache-2.0` | Permissive; Apache-2.0 covers service/network usage; OSS has no source-level dependency on the commercial plane. |
| the enterprise repository | Proprietary / commercial | No OSS source embedded; client never drives OSS code locally. |
| SDK (engine/mcp as library) | Permissive (MIT OR Apache-2.0) | Independent distribution; keeps client redistribution clean. |

### Enterprise-OSS connection = client/server API, not a library dependency
The connection is an HTTP JSON-RPC protocol: OSS implements the client, enterprise implements the server. Neither embeds the other. Standard SaaS boundary; avoids copyleft/derivative concerns even though OSS is permissive.

### Client-OSS licensing
- The OSS client (CLI + MCP + engine) is fully redistributable under MIT/Apache.
- The enterprise proxy inside OSS is a thin adapter under the OSS license and contains no enterprise IP.
- The proxy is disabled by default and requires explicit credentials; document this so users understand the default is offline.
- Protect enterprise via ToS + auth + rate limits, not via code restrictions in OSS.

## 6. Recommended actions

1. Verifier extended (done): `admin/verify_oss_standalone.sh` now also refuses `ENTERPRISE_API_URL`/`ENTERPRISE_API_KEY`.
2. Standardize or document the two env naming schemes (F1).
3. Harden TLS: remove or gate `ENTERPRISE_TLS_VERIFY=false` (F3).
4. CI gate: require `admin/verify_oss_standalone.sh` to pass on PRs touching the boundary.
5. Docs: add a "Licensing & boundary" section to README referencing this file.
6. Run full suites: `cargo test --workspace` (OSS) and `npm test` (enterprise) on a clean baseline once CI billing is fixed.
7. Optional: add an explicit `--offline`/`RIGORIX_OFFLINE=1` mode to MCP that skips enterprise proxy init entirely, making the offline guarantee machine-checkable.

