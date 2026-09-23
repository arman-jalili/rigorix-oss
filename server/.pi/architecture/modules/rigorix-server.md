# Module: rigorix-server

**Status:** Partial (PR B — transport + catalog + auth + version)
**Crate:** `server/` (`rigorix-server` binary)
**Issue:** #888 (OSS-C2)
**ADR:** rigorix-sdk ADR-0001 (native API catalog), D1/D3/D4/D5/D6/D7/D9/D10

## Purpose

The OSS **reference execution host**: a single-tenant HTTP host that serves the
frozen `rigorix.*` catalog (19 methods) over JSON-RPC 2.0 at `POST /rpc` and
(planned) SSE at `GET /events`. It is the first live target for the
`rigorix_enterprise_*` proxy seam.

It is a **host, not a second engine**: every method delegates to the same
composed host the stdio MCP binary uses (`rigorix_mcp::host::AppState::handle_tool_call`),
so method results are identical to the MCP tool counterparts (same DTOs, same
error taxonomy).

## Architecture

```
server/
├── src/
│   ├── main.rs        # axum bootstrap: POST /rpc, GET /health
│   ├── lib.rs         # module root
│   ├── backend.rs     # MethodBackend trait + HostBackend (delegates to rigorix-mcp)
│   ├── catalog.rs     # the frozen 19-method table (name -> auth -> mcp tool)
│   ├── rpc.rs         # JSON-RPC 2.0 codec: single/batch/notification + errors
│   └── version.rs     # rigorix.system.version
└── tests/             # rpc_transport, auth_levels, catalog_parity
```

## Contracts

| Concern | Contract |
|---------|----------|
| Transport | JSON-RPC 2.0; HTTP 200 for a well-formed envelope; notifications no body |
| Errors | `rigorix_mcp::host::error::HostError` → `errors.json` (`data.type`, codes) |
| Catalog | exactly the 19 frozen methods; MCP tools map 1:1 (ADR-0001 D9) |
| Auth | public / session / admin per catalog entry (D4) |
| Identity | session-derived (D5); the composed host injects the attested `IdentityRef` |
| Version | `rigorix.system.version` returns name/engine/api/schemas/capabilities |

## Delegation

`HostBackend::dispatch(method, params)` looks up the catalog entry and calls
`app_state().handle_tool_call(entry.mcp_tool, &params)`. `rigorix.system.version`
is handled by the server; `rigorix.policy.bundle` (admin, enterprise-only, no
MCP tool) returns `not_enabled` — the OSS engine is not the policy source of
truth.

`RIGORIX_REPO_ROOT` selects the repo; `rigorix_mcp::host::init_host` builds the
shared composition before the server binds (`RIGORIX_SERVER_BIND`, default
`127.0.0.1:3001`).

## Not yet implemented (planned PRs)

- **PR C** execution methods (already delegated; dedicated typed tests pending).
- **PR D** audit/template methods tests.
- **PR E** `GET /events` SSE (`approval_required`, `run_progress`,
  `policy_changed`; Last-Event-ID replay, heartbeat, backpressure). Blocked on
  SDK-C1. The legacy MCP SSE path is **not** resurrected (GAP-A-10).
