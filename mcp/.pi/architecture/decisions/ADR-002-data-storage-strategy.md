# ADR-002: Data Storage Strategy — SQLx + Per-Context Schema

**Status:** Superseded — SQLx was never adopted (0 `sqlx` deps/uses); storage is in-memory + filesystem repositories (mcp/src/*/infrastructure/, LocalAuditEnvelopeRepository)
**Date:** 2026-07-12
**Session:** d19b7a21-8f4c-4b3e-9a1d-5e6f7c8b9a0d

## Context

The MCP Gateway has diverse storage needs across its bounded contexts:

| Context | Data | Storage Pattern |
|---------|------|----------------|
| MCP Server | Session state, capability negotiation results | In-memory (ephemeral — sessions are runtime-only) |
| Execution Tools | None directly (delegates to rigorix-engine) | Engine-owned |
| Audit Tools | None directly (reads from rigorix-engine) | Engine-owned |
| Template Tools | TOML template files on disk | Filesystem (`.rigorix/templates/`) |
| Enterprise Proxy | Cached enterprise tool schemas | In-memory cache with TTL |

This means the MCP Gateway is **mostly stateless** — it bridges to existing rigorix-engine storage for execution and audit data. The two storage domains are:
1. **Template filesystem** — persistent, multi-user accessible
2. **In-memory state** — ephemeral, per-process

## Decision

1. **Templates**: Store as TOML files in `.rigorix/templates/` directory. Use atomic writes (temp-file + rename) for all mutations. No database needed.
2. **Enterprise schema cache**: In-memory `HashMap<String, ToolSchema>` with configurable TTL. No persistence across restarts (schemas are re-fetched on init).
3. **Sessions**: In-memory `HashMap<SessionId, Session>` with optional timeout-based eviction. No persistence.
4. **No SQLx/Postgres dependency** in the MCP Gateway itself. The gateway is a thin protocol bridge — rigorix-engine owns all persistent databases.

## Consequences

- **Positive**: Zero database dependencies — simpler build, smaller binary, faster startup
- **Positive**: Template filesystem is inherently portable across machines and AI tools
- **Positive**: No migration scripts, no connection pooling, no DB operations to monitor
- **Negative**: Template write concurrency must be handled via file locks
- **Negative**: No audit data caching — every audit tool call goes to rigorix-engine (acceptable — engine is local)
- **Negative**: Server restart clears all sessions (acceptable — clients reconnect on restart)

## Alternatives Considered

| Alternative | Rationale for Rejection |
|-------------|------------------------|
| Postgres via SQLx for all contexts | Over-engineered for a stateless protocol bridge; adds deployment complexity for zero benefit |
| SQLite for sessions | Session data is ephemeral and should not survive restarts; SQLite adds unnecessary I/O |
| Redis for schema cache | Single-machine MCP server doesn't need distributed caching; in-memory HashMap is faster and simpler |
| JSON files for templates | TOML is the rigorix standard for template format; consistency with engine conventions |

## Affected Modules

- Template Tools (filesystem storage)
- Enterprise Proxy (in-memory cache)
- MCP Server (in-memory sessions)
