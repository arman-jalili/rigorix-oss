# System Architecture Overview — Rigorix MCP Gateway

<!--
Canonical Reference: .pi/architecture/diagrams/system-overview.md
Blueprint Source: Guardian Framework v1.2
-->

## High-Level Architecture

The Rigorix MCP Gateway bridges AI coding assistants (Claude Code, Cursor, Aider, etc.) to the rigorix-engine via the Model Context Protocol (MCP). It is a modular monolith of **six bounded contexts** in a single Rust crate, all wired through in-process trait dispatch (ADR-003). The gateway is a thin adapter — zero business logic; the engine owns planning, execution, enforcement, audit, and identity attestation.

```
┌─────────────────────────────────────────────────────────────────┐
│                    AI Coding Assistants (MCP Clients)           │
│          Claude Code · Cursor · Aider · Cline · Continue.dev    │
└─────────────────────────────────────────────────────────────────┘
                              │
                    JSON-RPC 2.0 (stdio / SSE)
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                        MCP Server                               │
│        Protocol types · Transports · Session management         │
│        ToolRegistry · RequestRouter · Resource/Prompt providers │
└─────────────────────────────────────────────────────────────────┘
                              │
                 tool/call routing (prefix)
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                        Tool Handlers                            │
│  ┌────────────┐  ┌────────────┐  ┌────────────┐  ┌──────────┐  │
│  │ Execution  │  │  Audit     │  │ Template   │  │  Auth    │  │
│  │  Tools     │  │  Tools     │  │  Tools     │  │  Tools   │  │
│  └────────────┘  └────────────┘  └────────────┘  └──────────┘  │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │             Enterprise Proxy (feature-gated)             │  │
│  └──────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
                              │
        EngineFacade trait (in-process)          HTTPS JSON-RPC (Bearer)
                              │                              │
                              ▼                              ▼
┌─────────────────────────────────────────────────┐  ┌────────────────────┐
│              Rigorix Engine (local)             │  │  Rigorix Enterprise │
│  Orchestrator · DAG Executor · Enforcement      │  │  (optional)         │
│  Audit Storage · Identity Attestation           │  │  Cross-team audit,  │
└─────────────────────────────────────────────────┘  │  approvals, policies │
                                                     └────────────────────┘
```

## Module Layers

| Layer | Modules | Purpose | Entry Point |
|-------|---------|---------|-------------|
| Protocol | mcp-server | Transport, session lifecycle, tool registry, routing | `src/mcp_server/` |
| Tool Handlers | execution-tools, audit-tools, template-tools, auth, enterprise-proxy | MCP tool schemas + handlers per capability | `src/*/interfaces/mcp/` |

Each tool module follows DDD with Clean Architecture layers (domain → application → infrastructure → interfaces). The `interfaces/` layer is the MCP tool handler; `infrastructure/` holds the EngineFacade adapters and external clients.

## Module Dependency Graph

```
mcp-server
    ├── execution-tools    (rigorix_execute/run/plan/validate/approve)
    ├── audit-tools        (rigorix_read_audit/list_audits/audit_summary)
    ├── template-tools     (rigorix_list/get/create/validate_template)
    ├── auth               (rigorix_auth_login/status/logout)
    └── enterprise-proxy   (rigorix_enterprise_*, feature-gated)

execution-tools → rigorix-engine (EngineFacade: Orchestrator, ExecutionEnforcer)
audit-tools     → rigorix-engine (AuditService, read-only)
template-tools  → rigorix-engine (template repository, read/write)
auth            → rigorix-engine (IdentityAttestationService — attestation core)
enterprise-proxy→ rigorix-enterprise API (HTTPS JSON-RPC, Bearer token)
```

## Security Boundaries

| Boundary | Enforcement | Module |
|----------|-------------|--------|
| MCP Client → Gateway (stdio) | Trusted parent process — no auth (ADR-005) | mcp-server |
| MCP Client → Gateway (SSE localhost) | Localhost bind (default) — no auth | mcp-server |
| MCP Client → Gateway (SSE non-localhost) | Optional IdP/API-key gate when `mcp.sse.auth` set (ADR-008) | auth |
| Gateway → Engine | In-process trait calls; local trust | EngineFacade |
| Gateway → Enterprise | Bearer API key, HTTPS, TLS verification | enterprise-proxy |
| Human → Identity | Attributed claims via OIDC device flow; keychain custody (ADR-012) | auth |
| Agent tool calls | **Not** gated by auth — the client is the untrusted party (ADR-008) | — |

## Data Flow Overview

### Run with Identity (the flagship path)

```
Human → rigorix_auth_login (device flow) → short-TTL token (keychain refresh)
    → rigorix_run (identity attached) → engine Orchestrator → DAG execution
    → requires_approval step → PendingApproval
    → rigorix_approve_execution (approver_id + decision_context)
    → engine ApprovalService: intent hash → verify → dispatch → consume
    → HMAC-signed envelope (identity + approval_events + scope_violations)
    → rigorix_read_audit (evidence readback)
```

### Event Flow

Every module publishes lifecycle events to an in-process EventBus (`tokio::sync::broadcast`) for observability — see `diagrams/event-flow.md`. Events never carry operational logic.

## Key Integration Points

| Integration | Protocol | Module |
|-------------|----------|--------|
| AI assistants | MCP (JSON-RPC 2.0) over stdio/SSE | mcp-server |
| Rigorix engine | In-process EngineFacade trait | execution/audit/template/auth |
| Rigorix Enterprise | HTTPS JSON-RPC + Bearer token | enterprise-proxy |
| Identity Provider | OIDC device flow (RFC 8628) | auth |

---

*Last updated: 2026-08-28*
*Replaces the previous generic scaffold (which described an api-gateway/auth-system architecture that does not exist in this crate)*
