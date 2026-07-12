# ADR-005: Authentication & Authorization — Transport-Level + Enterprise Proxy Auth

**Status:** Accepted
**Date:** 2026-07-12
**Session:** d19b7a21-8f4c-4b3e-9a1d-5e6f7c8b9a0d

## Context

The MCP Gateway operates in two distinct security contexts:

1. **OSS mode** (no enterprise): The gateway runs on the developer's local machine. Security relies on:
   - **stdio**: Trusted parent process (the AI coding tool) controls stdin/stdout
   - **SSE**: Bound to localhost (127.0.0.1) by default; only local processes can connect

2. **Enterprise mode**: The gateway additionally communicates with the Rigorix Enterprise API:
   - Requires authentication via Bearer token (API key)
   - API key must never be logged or leaked
   - HTTPS with TLS verification is mandatory

The MCP protocol itself has no built-in authentication — it relies on transport-level security. For stdio, the parent process inherits the OS user's identity. For SSE, binding to localhost provides network-level access control.

## Decision

1. **Stdio transport security**: No additional authentication. The parent AI process must be trusted (the developer chose to configure it).
2. **SSE transport security**:
   - Default bind: `127.0.0.1` only (localhost)
   - Configurable via `mcp.sse.bind_address` setting
   - Warning logged if binding to non-localhost addresses
3. **Enterprise API authentication**:
   - API key stored as `Secret` type (redacted Debug/Display/Serialize)
   - Configured via `enterprise.api_key` in rigorix config or `RIGORIX_ENTERPRISE_API_KEY` env var
   - Sent as `Authorization: Bearer <api_key>` header on all enterprise API calls
   - HTTPS enforced — TLS verification is mandatory with optional `tls_verify: false` override
4. **No RBAC in OSS gateway**: Role-based access control is an enterprise server concern.
5. **No audit MCP authentication**: Audit data is read-only from rigorix-engine; engine enforces its own audit auth if needed.

## Consequences

- **Positive**: Zero security overhead for stdio/SSE localhost usage — matches how all MCP tools work
- **Positive**: Enterprise API key is never leaked in logs (Secret type)
- **Positive**: Clear security boundary — OSS tools need no auth config
- **Negative**: SSE bound to non-localhost is inherently less secure (documented warning)
- **Negative**: No per-user authorization in OSS — any local process that can connect to SSE port can use all tools
- **Negative**: Enterprise API key management is out of scope for OSS (key rotation, scoping)

## Alternatives Considered

| Alternative | Rationale for Rejection |
|-------------|------------------------|
| MCP-level authentication extension | MCP spec has no standard auth mechanism; custom auth would break compatibility with AI tools |
| API key for stdio as well | Stdio parent process already controls the pipe; adding an API key adds complexity for no security gain |
| OAuth2 for SSE clients | Over-engineered for a localhost service; no OAuth2 provider integration on the MCP client side |
| mTLS for SSE | Users would need to generate and manage certificates; dramatically increases setup friction |

## Affected Modules

- MCP Server (transport binding configuration)
- Enterprise Proxy (Secret type, Bearer token auth, HTTPS enforcement)
