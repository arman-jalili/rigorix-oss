# rigorix-server

**The OSS reference execution host.** A single-tenant HTTP host that serves the
frozen `rigorix.*` catalog over JSON-RPC 2.0, streams execution events over SSE,
and optionally speaks MCP Streamable HTTP.

```bash
cargo run -p rigorix-server
```

> **It is a host, not a second engine.** Every method delegates to the same
> composed host the stdio MCP binary uses
> (`rigorix_mcp::host::AppState::handle_tool_call`). A method called over
> `/rpc` and the equivalent MCP tool called over stdio return **identical
> results** — same DTOs, same error taxonomy, same enforcement. There is one
> implementation of every rule, and this crate is not it.

---

## Surfaces

| Route | Method | What it is |
|---|---|---|
| `/rpc` | `POST` | JSON-RPC 2.0 over the frozen catalog — single requests, batch arrays, notifications |
| `/events` | `GET` | Server-Sent Events: the live execution event stream (the engine event bus is bridged into the hub) |
| `/mcp` | `POST` / `GET` / `DELETE` | The optional **MCP Streamable HTTP** adapter (ADR-0001 D4) |
| `/health` | `GET` | Liveness |

The MCP adapter is the **same governed dispatch and the same auth** as `/rpc`. It
is a second transport for one surface, not a second surface — governed tools are
derived from the catalog, not hand-listed.

### The catalog

The server implements a **subset** of the single closed `rigorix.*` namespace
(ADR-0001 D3): it may not extend the namespace. Currently **20 methods**, of which
**18** are exposed as governed MCP tools.

`rigorix.policy.bundle` is admin-only and has no MCP tool; the OSS host refuses it
(`not_enabled`) rather than fabricating a local policy export, because the engine
is not the policy source of truth.

**Subset enforcement is real, not aspirational.** `server/tests/catalog_parity.rs`
checks two relations against the published SDK catalog:

- every in-code method must be **declared** there, with matching `auth` and
  `mcpTool` — never a set or count equality, because the SDK catalog is
  deliberately larger (it declares the enterprise-side `rigorix.auth.verify`,
  which OSS does not implement);
- the catalog's `mcpTool` values must map **one-to-one** onto the tools this host
  registers.

That second guard means a catalog entry and its implementing tool cannot land
separately — landing either half alone is a red build.

### Error taxonomy

Transport-level failures are JSON-RPC error objects, never HTTP status games. A
well-formed envelope always yields HTTP 200; method-level failures carry the
taxonomy from `errors.json`:

| Code | Meaning |
|---|---|
| `-32700` | Parse error — the body is not JSON |
| `-32600` | Invalid request — structurally invalid envelope, empty batch |
| `-32601` | Method not found |
| `-32602` | Invalid params |
| `-32603` | Internal error |
| `-32001` | No attested session |
| `-32002` | Session lacks scope |

### Auth

Three levels (ADR-0001 D4): `public`, `session`, `admin`.

`session` methods require an attested session **when an identity provider is
configured** (`RIGORIX_IDP_ISSUER` + `RIGORIX_IDP_CLIENT_ID`). Without an IdP the
host runs in local mode and does not refuse them — the same posture as the rest of
the OSS surface, and it is deliberate rather than accidental: identity attestation
is a *marker*, not a local authorization gate (ADR-012).

---

## Audit anchoring

`server/src/anchor` composes the engine's shared `AnchorRuntime` for ADR-016's
ledger tier: it exposes the active anchor mode and the verified head for
`rigorix.system.version`. **The host never holds the anchor's private key** — the
anchor is an external, agent-inaccessible authority, and the OSS host only reports
what it can verify. That constraint is the reason the tier exists (ADR-016 Phase D).

---

## Architecture

Flat by design — this is a transport and composition crate, so it has no bounded
contexts of its own:

| File | Responsibility |
|---|---|
| `main.rs` | axum bootstrap and routing |
| `lib.rs` | module root |
| `rpc.rs` | JSON-RPC 2.0 codec: single / batch / notification + error objects |
| `backend.rs` | `MethodBackend` trait + `HostBackend` — the delegation seam to `rigorix-mcp` |
| `catalog.rs` | The frozen catalog table: `name` → `auth` → `mcp_tool` |
| `mcp_http.rs` | MCP Streamable HTTP: sessions, `Origin` guard, governed tool list |
| `events.rs` | The SSE hub (bounded replay buffer; a slow consumer lags, never stalls execution) |
| `event_bridge.rs` | Bridges the engine's event bus into the hub |
| `state.rs` | Server state |
| `version.rs` | `rigorix.system.version` |
| `anchor/` | The ADR-016 anchor surface |

`MethodBackend` is why the transport is testable without a live engine: the RPC
codec, catalog rules and auth levels are exercised against stub backends, and
`HostBackend` is the only production implementation.

Note the delegation direction, because it is easy to get backwards: the server
does **not** implement a method and expose a tool. It looks up the catalog entry
and routes the call to the MCP tool named in `entry.mcp_tool`. The MCP host is
where the behaviour lives.

---

## Development

```bash
cargo test -p rigorix-server
cargo clippy -p rigorix-server -- -D warnings
cargo run -p rigorix-server
bash .pi/scripts/validate-architecture.sh
```

Architecture: [`.pi/architecture/modules/rigorix-server.md`](.pi/architecture/modules/rigorix-server.md).

---

## License

Licensed under either of MIT ([LICENSE-MIT](../LICENSE-MIT)) or Apache-2.0
([LICENSE-APACHE](../LICENSE-APACHE)), at your option.
