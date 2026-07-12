# ADR-001: Domain-Driven Design with Bounded Contexts

**Status:** Accepted
**Date:** 2026-07-12
**Session:** d19b7a21-8f4c-4b3e-9a1d-5e6f7c8b9a0d

## Context

The domain exploration identified five bounded contexts that represent distinct functional areas with separate responsibilities, data ownership, and evolution rates:

1. **MCP Server** — Core protocol implementation (transport, session, routing)
2. **Execution Tools** — Bridge to rigorix-engine for plan execution
3. **Audit Tools** — Bridge to rigorix-engine audit subsystem
4. **Template Tools** — Filesystem-backed template management
5. **Enterprise Proxy** — Dynamic proxy to Rigorix Enterprise API

Each context has different:
- **Change drivers**: MCP spec updates vs. engine API changes vs. enterprise feature releases
- **Data ownership**: Runtime state (MCP sessions) vs. engine queries (audit/execution) vs. filesystem (templates) vs. proxied API (enterprise)
- **Failure domains**: Session failure should not affect execution; enterprise failure should not affect OSS tools

## Decision

We will use **Domain-Driven Design with bounded contexts as independently evolvable modules** (Modular Monolith pattern) with the following architectural rules:

1. **Module isolation**: Each bounded context is a Rust crate with its own `domain/`, `application/`, `infrastructure/`, and `interfaces/` layers
2. **Language-level boundary**: Cross-context communication uses Rust traits + dependency injection at the binary composition root
3. **No shared databases**: Each context that requires persistence owns its schema; cross-context queries go through domain service interfaces
4. **Enterprise proxy isolation**: The enterprise proxy is conditionally compiled via Cargo feature flags — zero enterprise code in the OSS binary unless explicitly enabled

## Consequences

- **Positive**: Each bounded context can be tested, evolved, and deployed independently
- **Positive**: Enterprise Proxy can be conditionally compiled, keeping OSS binary clean
- **Positive**: Clear ownership boundaries make it easy to extract a context to a separate microservice later
- **Negative**: Cross-context calls require defining shared kernel interfaces (trait contracts)
- **Negative**: Compile-time dependency graph must be enforced by Cargo workspace dependencies (no circular deps)
- **Negative**: The binary composition root must wire all contexts together, adding complexity

## Alternatives Considered

| Alternative | Rationale for Rejection |
|-------------|------------------------|
| Monolith without domain boundaries | No separation of concerns; MCP protocol changes could affect execution logic |
| Microservices from day one | Over-engineering for initial scope; adds network overhead, deployment complexity, and operational burden for a single-machine MCP server |
| Layered Architecture (presentation/domain/data) | Does not enforce domain boundaries; cross-cutting concerns (audit, execution) would leak across layers |

## Affected Modules

- MCP Server (crate: `rigorix-mcp-server`)
- Execution Tools (crate: `rigorix-execution-tools`)
- Audit Tools (crate: `rigorix-audit-tools`)
- Template Tools (crate: `rigorix-template-tools`)
- Enterprise Proxy (crate: `rigorix-enterprise-proxy`, feature-gated)
- Binary composition crate (`rigorix-mcp` or `rigorix-mcp-bin`)
