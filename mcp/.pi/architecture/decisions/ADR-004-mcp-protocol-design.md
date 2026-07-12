# ADR-004: MCP Protocol Design — Hand-Rolled JSON-RPC over stdio/SSE

**Status:** Accepted
**Date:** 2026-07-12
**Session:** d19b7a21-8f4c-4b3e-9a1d-5e6f7c8b9a0d

## Context

The MCP Gateway communicates with AI coding assistants via the Model Context Protocol (MCP). MCP is a JSON-RPC 2.0 based protocol with the following message types:

1. **Initialize / Initialized** — Session capability negotiation
2. **tools/list** — Client discovers available tools
3. **tools/call** — Client invokes a tool
4. **resources/read** — Client reads a resource URI
5. **resources/list** — Client discovers available resources
6. **prompts/get** — Client requests a prompt template
7. **prompts/list** — Client discovers available prompts
8. **notifications/** — Asynchronous events (progress, cancellation)

Transport options:
- **stdio** — stdin/stdout, used by CLI-bundled AI tools (Claude Code, Aider)
- **SSE** — HTTP Server-Sent Events, used by GUI tools (Claude Desktop, Cursor)

Per the domain exploration's resolved decisions, the MCP protocol implementation is hand-rolled (no external MCP SDK) using ~200 lines of core JSON-RPC types, with Axum for SSE transport.

## Decision

1. **No external MCP SDK**: Hand-roll JSON-RPC message types. The protocol is simple and stable enough that an SDK adds dependency risk without meaningful value.
2. **Axum for SSE transport**: Axum is the standard Rust async HTTP framework with first-class SSE support via `axum::response::Sse`.
3. **stdio transport**: Direct stdin/stdout read/write with newline-delimited JSON. No framing overhead.
4. **Tool naming convention**: All OSS tools use the `rigorix_` prefix (e.g., `rigorix_execute`). Enterprise tools use `rigorix_enterprise_*` prefix.
5. **Resource URIs**: Use `rigorix://` scheme (e.g., `rigorix://audit/{id}`, `rigorix://templates/{name}`).
6. **Protocol version**: Support `2025-03-26` MCP spec version during initialization.
7. **Progress notifications**: SSE only. Stdio is synchronous and progress messages would interleave with tool responses.

## Consequences

- **Positive**: Zero upstream dependency risk — no MCP SDK version conflicts
- **Positive**: Full control over JSON-RPC message serialization (serde-based, no surprises)
- **Positive**: Axum provides mature SSE implementation with backpressure support
- **Negative**: We own MCP spec compliance testing ourselves
- **Negative**: If MCP spec adds major new features (e.g., streaming responses), we may need to adopt an SDK later
- **Negative**: No automatic MCP transport negotiation — must implement both stdio and SSE manually

## Alternatives Considered

| Alternative | Rationale for Rejection |
|-------------|------------------------|
| MCP Rust SDK (e.g., `mcp-sdk`) | No stable Rust MCP SDK exists as of 2026; risk of unmaintained or breaking APIs |
| gRPC instead of JSON-RPC | MCP spec requires JSON-RPC; gRPC would be non-compliant |
| Tungstenite (WebSocket) for SSE | MCP spec defines SSE, not WebSocket; SSE is simpler for read-only streaming |
| Embedded HTTP server (hyper directly) | Axum provides higher-level SSE primitives; hyper would require more boilerplate |

## Affected Modules

- MCP Server (protocol implementation)
- All tool handlers (must implement MCP tool JSON schemas)
