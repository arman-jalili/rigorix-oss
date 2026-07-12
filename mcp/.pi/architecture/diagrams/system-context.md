# System Context Diagram

## Overview

The Rigorix MCP Gateway bridges AI coding assistants to the Rigorix execution engine via the Model Context Protocol (MCP). It operates as a modular monolith with five bounded contexts.

## Bounded Contexts Flow

```mermaid
graph TB
    subgraph "AI Coding Assistants (MCP Clients)"
        AC1["Claude Desktop"]
        AC2["Claude Code"]
        AC3["Cursor"]
        AC4["Cline (VS Code)"]
        AC5["Continue.dev"]
        AC6["Aider"]
        AC7["Copilot Codex"]
    end

    subgraph "Rigorix MCP Gateway"
        MCP["MCP Server\n(Protocol, Transport, Routing)"]

        subgraph "Tool Handlers"
            EXEC["Execution Tools\n(rigorix_execute)"]
            AUDIT["Audit Tools\n(rigorix_read_audit)"]
            TEMPL["Template Tools\n(rigorix_list_templates)"]
            EP["Enterprise Proxy\n(rigorix_enterprise_*)"]
        end
    end

    subgraph "Rigorix Engine (local)"
        ENG["Rigorix Engine\n(Orchestrator, DAG Executor,\n Enforcement, Audit Storage)"]
    end

    subgraph "Rigorix Enterprise (optional)"
        ENT["Enterprise Server\n(Cross-team Audit,\n Approval Workflows,\n Policy Management)"]
    end

    AC1 & AC2 & AC3 & AC4 & AC5 & AC6 & AC7 -->|"JSON-RPC over stdio/SSE"| MCP
    MCP -->|"tool/call routing"| EXEC
    MCP -->|"tool/call routing"| AUDIT
    MCP -->|"tool/call routing"| TEMPL
    MCP -->|"if enterprise configured"| EP

    EXEC -->|"EngineFacade trait"| ENG
    AUDIT -->|"EngineFacade trait"| ENG
    TEMPL -->|"EngineFacade trait"| ENG

    EP -->|"HTTPS JSON-RPC\n(Bearer token auth)"| ENT

    style MCP fill:#4a90d9,stroke:#2c5f8a,color:#fff
    style EXEC fill:#50b080,stroke:#2d6a4f,color:#fff
    style AUDIT fill:#e9c46a,stroke:#b8973a,color:#222
    style TEMPL fill:#f4a261,stroke:#c1773a,color:#222
    style EP fill:#e76f51,stroke:#b5452a,color:#fff
    style ENG fill:#6b5b95,stroke:#4a3d6e,color:#fff
    style ENT fill:#b56576,stroke:#803f4e,color:#fff
```

## Context Boundaries

| Boundary | Includes | Communication |
|----------|----------|---------------|
| **AI Tools** | MCP clients (7+ different tools) | JSON-RPC 2.0 over stdio or SSE |
| **MCP Gateway** | 5 bounded contexts in a Rust crate workspace | In-process trait calls + EventBus |
| **Rigorix Engine** | Orchestrator, DAG executor, enforcement, audit | Local engine API calls via EngineFacade trait |
| **Rigorix Enterprise** | Multi-team audit, approvals, policies | HTTPS JSON-RPC with Bearer token auth |

---

*Generated from session: d19b7a21-8f4c-4b3e-9a1d-5e6f7c8b9a0d*
*Updated: 2026-07-12*
