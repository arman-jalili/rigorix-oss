# feat: #915 — optional MCP Streamable HTTP adapter

Implements **#915** (`OSS-MCP-STREAMABLE-HTTP`, ADR-0001 **D4**).

> **Trigger:** the issue is explicitly trigger-gated ("do not build
> speculatively") on a demonstrated GUI-agent need. This PR was opened on an
> explicit implementation instruction (trigger treated as confirmed); flag it
> if that is not the case.

## What

An MCP Streamable HTTP endpoint on `rigorix-server`, mounted at **`POST /mcp`**,
as a thin adapter over the frozen `rigorix.*` catalog. No new `rigorix.*`
method; no raw-step surface.

| Method | Behavior |
|---|---|
| `POST /mcp` | One JSON-RPC message. Requests → JSON (`application/json`) or request-scoped SSE (`text/event-stream`, chosen by `Accept`); notifications/responses → `202 Accepted`. |
| `GET /mcp` | `405 Method Not Allowed` (spec-permitted; live run events stay on `GET /events`). |
| `DELETE /mcp` | Terminates the session (`204`). |

- **Sessions:** `initialize` issues `Mcp-Session-Id`; subsequent requests MUST
  present it (else `404`); `notifications/initialized` marks it ready.
- **Security:** `Origin` validated (DNS-rebinding), missing/`null` allowed for
  non-browser clients.

## Governance

- **Catalog parity.** `tools/list` returns exactly the catalog's governed
  subset — the 17 non-admin methods with an MCP tool. Descriptors are the same
  ones the stdio host advertises; enterprise-proxy tools, `rigorix.policy.bundle`
  (admin), and `rigorix.system.version` (host-only) are never exposed.
- **No raw-step escape hatch** (D6/D10). A `tools/call` name is accepted only if
  it maps to a catalog entry; the arguments become the catalog params.
- **Same auth as `/rpc`.** `tools/call` routes through the extracted
  `rpc::invoke` — the single governed path behind both transports — so
  `public`/`session`/`admin` decisions are identical by construction. Tool
  failures are MCP `isError` results (matching the stdio host).

## Files

- `server/src/mcp_http.rs` (new) — session store, governance, MCP dispatch,
  axum handlers, SSE/JSON negotiation, `Origin` guard.
- `server/src/rpc.rs` — extract `invoke` (shared governed dispatch/a​uth path).
- `server/src/{state,lib,main,version}.rs` — session store on state, `/mcp`
  routes, `mcp-streamable-http` capability.
- `server/tests/mcp_streamable_http.rs` (new) — in-process axum integration.
- `docs/sdk-server-strategy-freeze-2026-09-06.md` — D-4 status.

## Acceptance criteria

| # | Criterion | Status |
|---|-----------|--------|
| 1 | A GUI agent can drive governed flows over MCP Streamable HTTP | ✅ integration test drives `initialize` → `tools/list` → `tools/call` over the real router |
| 2 | No raw-step escape hatch; catalog parity preserved; same auth as `/rpc` | ✅ conformance test + auth-parity test |
| 3 | CI green | ✅ pending CI on this PR |

## Validation

```
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
RIGORIX_SDK_SCHEMAS=<sdk>/schemas cargo test -p rigorix-server --test catalog_parity
cargo test -p rigorix-engine --features conformance --test conformance
bash .pi/scripts/validate-ci.sh validate-tests.sh validate-security.sh validate-operations.sh
```

All green locally.

Refs #915 · ADR-0001 D4/D6/D10 · GAP-A-10 (legacy MCP SSE not resurrected)
