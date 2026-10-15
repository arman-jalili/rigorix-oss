# rigorix-mcp

**The governed MCP surface.** `rigorix-mcp` is the stdio MCP server that plugs
Rigorix into any MCP-compatible coding assistant — and it is also the shared tool
host that `rigorix-server` delegates to. **Both adapters dispatch through the same
`AppState::handle_tool_call`**, so a tool called over stdio and the equivalent
catalog method called over JSON-RPC return identical results with the same DTOs
and the same error taxonomy. There is no second implementation to drift.

```bash
cargo install rigorix-mcp
```

Add to `.mcp.json` in your project root:

```json
{
  "mcpServers": {
    "rigorix-mcp": { "command": "rigorix-mcp", "args": [] }
  }
}
```

Your agent keeps its own loop. Everything it routes through Rigorix is planned,
gated and recorded.

---

## Tools

**15 built-in OSS tools**, with names frozen by the `rigorix.*` catalog:

| Group | Count | Tools |
|---|---|---|
| **Execution** | 6 | `rigorix_execute`, `rigorix_run`, `rigorix_plan`, `rigorix_validate_plan`, `rigorix_check_enforcement`, `rigorix_approve_execution` |
| **Templates** | 4 | `rigorix_list_templates`, `rigorix_get_template`, `rigorix_create_template`, `rigorix_validate_template` |
| **Audit** | 4 | `rigorix_read_audit`, `rigorix_list_audits`, `rigorix_audit_summary`, `rigorix_audit_authority_facts` |
| **Usage guide** | 1 | `rigorix_get_usage_guide` |

Two things are worth calling out.

`rigorix_audit_authority_facts` exports an **authority-fact projection bundle** for
offline third-party verification. Its description states the disclosure explicitly,
because it is inherent to the design: **a bundle carries the signed envelopes** —
the caller receives the history, not a summary. Verification is by re-derivation
(spec §4.3.1), not by trusting a signature.

`rigorix_approve_execution` is how a human approves a step that the engine gated.
The approval binds to a canonical `ExecutionIntent`, not to a node, and is
re-verified at dispatch (ADR-011) — see [`engine/README.md`](../engine/README.md).

**3 auth tools** — `rigorix_auth_login`, `rigorix_auth_status`,
`rigorix_auth_logout` — are appended to the **stdio** surface only when an identity
provider is configured (`RIGORIX_IDP_ISSUER` + `RIGORIX_IDP_CLIENT_ID`); without
one they are absent from `tools/list` rather than present and failing. On the
**server** surface they are ordinary catalog entries at `auth: public`, because
starting a login is the call you make *before* you have a session.

---

## Modules

8 bounded-context modules, each with its own architecture document:

| Module | Responsibility |
|---|---|
| `mcp_server` | Protocol surface: `initialize`, `tools/list`, `tools/call`, transport |
| `host` | The composition root — `AppState`, tool routing, `all_tool_descriptors()` |
| `execution_tools` | Plan/run/validate/approve handlers over the engine facade |
| `template_tools` | Template CRUD over the filesystem repository |
| `audit_tools` | Read-only audit queries, formatting, and the authority-fact bundle |
| `usage_guide` | The self-documenting tool list and workflow patterns |
| `auth` | Session tool handlers (conditional — see above) |
| `enterprise_proxy` | **Conditional** enterprise API-gateway seam |

### The enterprise proxy is conditional by construction

When no enterprise configuration is present, zero enterprise code is loaded and no
`rigorix_enterprise_*` tool appears in the list. It is not "present but disabled".

---

## Transports

| Transport | Where | Notes |
|---|---|---|
| **stdio** | `rigorix-mcp` binary | The default. What a coding assistant spawns |
| **MCP Streamable HTTP** | `rigorix-server` (`server/src/mcp_http.rs`) | Optional adapter over the same host; sessions and `Origin` guard |
| **JSON-RPC 2.0 + SSE** | `rigorix-server` | `/rpc` and `/events` over the frozen catalog |

See [`server/README.md`](../server/README.md) for the HTTP surfaces.

---

## Architecture

```
mcp/src
├── mcp_server/       protocol surface (initialize, tools/list, tools/call)
├── host/             AppState — the composition root; ALL tool routing
├── execution_tools/  ┐
├── template_tools/   ├─ one bounded context each:
├── audit_tools/      │  domain / application / infrastructure / interfaces
├── usage_guide/      │
├── auth/             │
└── enterprise_proxy/ ┘
```

Each tool follows the same shape: an input **JSON Schema** constant, a
**descriptor** function returning `{ name, description, inputSchema }`, a handler
**trait** in `application/service.rs`, an **impl** in `application/service_impl.rs`,
and a **dispatch arm** in `host/mod.rs`.

**Tool names are part of the published contract.** They come from the frozen
`rigorix.*` catalog and must not be renamed. Two guards enforce the seam:
`mcp/tests/schema_descriptor_tests.rs` pins the descriptor set, and
`server/tests/catalog_parity.rs` asserts an exact one-to-one mapping between the
catalog's `mcpTool` values and the tools this host registers — which is why the
catalog entry and the tool implementing it cannot be landed separately.

---

## Development

```bash
cargo test -p rigorix-mcp
cargo clippy -p rigorix-mcp -- -D warnings
cargo run -p rigorix-mcp                       # stdio server
bash .pi/scripts/validate-canonical.sh
```

Architecture: [`.pi/architecture/modules/`](.pi/architecture/modules/).

---

## License

Licensed under either of MIT ([LICENSE-MIT](../LICENSE-MIT)) or Apache-2.0
([LICENSE-APACHE](../LICENSE-APACHE)), at your option.
