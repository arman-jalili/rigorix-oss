# Rigorix MCP Gateway

[![Crates.io](https://img.shields.io/crates/v/rigorix-mcp)](https://crates.io/crates/rigorix-mcp)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-green)](https://github.com/arman-jalili/rigorix-oss)
[![Documentation](https://docs.rs/rigorix-mcp/badge.svg)](https://docs.rs/rigorix-mcp)
[![CI](https://github.com/arman-jalili/rigorix-oss/actions/workflows/ci.yml/badge.svg)](https://github.com/arman-jalili/rigorix-oss/actions/workflows/ci.yml)

**Model Context Protocol (MCP) gateway server for the Rigorix engine.**

`rigorix-mcp` bridges AI coding assistants (Claude Code, Cursor, Aider, and any MCP-compatible client) with the Rigorix engine. It exposes 14 built-in OSS tools for plan execution, template management, and audit queries — plus optional enterprise proxy tools for API gateway integration.

---

## Quick Start

```bash
# Install from crates.io
cargo install rigorix-mcp

# Run in stdio mode (default, for Claude Code / Aider)
rigorix-mcp

# Run in SSE mode (for Claude Desktop / Cursor)
rigorix-mcp --sse --bind 127.0.0.1:3001
```

The server reads newline-delimited JSON-RPC messages from stdin (stdio mode) or accepts HTTP SSE connections (SSE mode). No configuration file required for basic use.

---

## Architecture

```
┌──────────────────────────────────────────────────────────────┐
│                     MCP Client (AI Tool)                      │
│  Claude Code · Cursor · Aider · Any MCP-compatible client    │
└──────────┬─────────────────────────────────────┬─────────────┘
           │ JSON-RPC (stdio or SSE)              │
           ▼                                      ▼
┌──────────────────────────────────────────────────────────────┐
│                    rigorix-mcp (Server)                       │
│                                                              │
│  ┌─────────────┐  ┌──────────────┐  ┌──────────────────┐    │
│  │  Transport   │  │   Session    │  │  Request Router   │    │
│  │ stdio / SSE  │──│   Manager    │──│  (prefix-based)   │    │
│  └─────────────┘  └──────────────┘  └────────┬─────────┘    │
│                                               │              │
│                    ┌──────────────────────────┼──────────┐   │
│                    │        Tool Handlers     │          │   │
│                    │                          ▼          │   │
│                    │  ┌─────────────────────────────┐   │   │
│                    │  │  Execution Tools (6)         │   │   │
│                    │  │  rigorix_execute             │   │   │
│                    │  │  rigorix_run                 │   │   │
│                    │  │  rigorix_plan                │   │   │
│                    │  │  rigorix_validate_plan       │   │   │
│                    │  │  rigorix_check_enforcement   │   │   │
│                    │  │  rigorix_approve_execution   │   │   │
│                    │  └─────────────────────────────┘   │   │
│                    │  ┌─────────────────────────────┐   │   │
│                    │  │  Template Tools (4)          │   │   │
│                    │  │  rigorix_list_templates      │   │   │
│                    │  │  rigorix_get_template        │   │   │
│                    │  │  rigorix_create_template     │   │   │
│                    │  │  rigorix_validate_template   │   │   │
│                    │  └─────────────────────────────┘   │   │
│                    │  ┌─────────────────────────────┐   │   │
│                    │  │  Audit Tools (3)             │   │   │
│                    │  │  rigorix_read_audit          │   │   │
│                    │  │  rigorix_list_audits         │   │   │
│                    │  │  rigorix_audit_summary       │   │   │
│                    │  └─────────────────────────────┘   │   │
│                    │  ┌─────────────────────────────┐   │   │
│                    │  │  Usage Guide (1)             │   │   │
│                    │  │  rigorix_get_usage_guide     │   │   │
│                    │  └─────────────────────────────┘   │   │
│                    └────────────────────────────────────┘   │
│                                                              │
│  ┌────────────────────────┐   ┌─────────────────────────┐   │
│  │  Enterprise Proxy      │   │  Schema Cache            │   │
│  │  (optional,            │   │  (discovery + TTL)       │   │
│  │   rigorix_enterprise_*)│   │                          │   │
│  └────────────────────────┘   └─────────────────────────┘   │
│                                                              │
│  ┌──────────────────────────────────────────────────────┐   │
│  │  EngineFacade → rigorix-engine (local process)       │   │
│  └──────────────────────────────────────────────────────┘   │
└──────────────────────────────────────────────────────────────┘
```

### Bounded Contexts

The crate follows Domain-Driven Design with Clean Architecture layers inside each bounded context:

| Context | Responsibility | Tools |
|---------|---------------|-------|
| **MCP Server** | Transport, sessions, tool registry, request routing | Foundation layer |
| **Execution Tools** | Plan execution, validation, enforcement checking, human sign-off | 6 tools |
| **Template Tools** | Template CRUD (TOML files on disk) | 4 tools |
| **Audit Tools** | Read-only audit querying and formatting | 3 tools |
| **Usage Guide** | Self-documenting tool list and workflow patterns | 1 tool |
| **Enterprise Proxy** | Conditional enterprise API gateway (optional) | 2 tools |

---

## Tools Reference

### Execution Tools

| Tool | Description |
|------|-------------|
| `rigorix_execute` | Execute a plan (DAG) through the Rigorix engine |
| `rigorix_run` | Load a template and execute its DAG through the engine |
| `rigorix_plan` | Load a template and display the planned DAG without execution |
| `rigorix_validate_plan` | Validate a plan against enforcement policies |
| `rigorix_check_enforcement` | Check if any enforcement policies are active |
| `rigorix_approve_execution` | Approve steps of a paused (`PendingApproval`) execution — the human sign-off for `requires_approval` steps |

### Template Tools

| Tool | Description |
|------|-------------|
| `rigorix_list_templates` | List available templates (with optional search filter) |
| `rigorix_get_template` | Get a template's content (TOML or JSON format) |
| `rigorix_create_template` | Create or overwrite a template |
| `rigorix_validate_template` | Validate template structure (steps, parameters, dependencies) |

Templates are stored as TOML files in `.rigorix/templates/` relative to the working directory.

### Audit Tools

| Tool | Description |
|------|-------------|
| `rigorix_read_audit` | Read an audit record by execution ID |
| `rigorix_list_audits` | List recent audit records (with status/time filters) |
| `rigorix_audit_summary` | Generate aggregate audit statistics over a time range |

All audit operations are read-only — the gateway never creates or modifies audit data.

### Usage Guide

| Tool | Description |
|------|-------------|
| `rigorix_get_usage_guide` | Returns structured context about action types, workflow patterns, and plan JSON structure |

This self-documenting tool helps AI assistants understand how to use Rigorix correctly.

---

## Transport Modes

### stdio Mode (Default)

```bash
rigorix-mcp
```

- Reads JSON-RPC from stdin, writes to stdout
- Designed for AI coding tools (Claude Code CLI, Aider)
- Process management by the parent AI tool

### SSE Mode

```bash
rigorix-mcp --sse --bind 127.0.0.1:3001
```

- HTTP server with Server-Sent Events streaming
- Designed for GUI tools (Claude Desktop, Cursor)
- Exposes a `/health` endpoint: `GET /health`

---

## Configuration

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `RIGORIX_REPO_ROOT` | `.` (CWD) | Repository root path for template/config discovery |
| `ENTERPRISE_API_URL` | — | Enterprise API base URL (enables proxy) |
| `ENTERPRISE_API_KEY` | — | Enterprise API authentication key |
| `ENTERPRISE_TIMEOUT_SECS` | 30 | Enterprise request timeout |
| `ENTERPRISE_TLS_VERIFY` | true | TLS certificate verification |
| `ENTERPRISE_MAX_RETRIES` | 3 | Max retries on transient errors |
| `ENTERPRISE_SCHEMA_TTL_SECS` | 3600 | Schema cache TTL |

### TOML Configuration File

`rigorix.toml` in the repo root:

```toml
# Audit backend (optional — audit envelopes sent via HTTP)
audit_backend_url = "https://<backend-url>
audit_backend_key = "rgx_live_sk_..."

# Optional sections with defaults — all keys are optional
[orchestrator]
# control parallelism, retries, timeouts

```

Audit config is loaded from `rigorix.toml`. Enterprise proxy is configured via environment variables only.

### Claude Code Configuration

Add to `.mcp.json` in your project root:

```json
{
  "mcpServers": {
    "rigorix-mcp": {
      "command": "rigorix-mcp",
      "args": [],
      "env": {}
    }
  }
}
```

Or with a full path to the binary:

```json
{
  "mcpServers": {
    "rigorix-mcp": {
      "command": "/Users/<you>/.cargo/bin/rigorix-mcp",
      "args": [],
      "env": {}
    }
  }
}
```

No environment variables are required — the binary reads `rigorix.toml` from the current working directory for audit config, and auto-derives `repository` (from `git remote get-url origin`) and `author` (from `git config user.email`) at execution time.

### Other MCP Clients

Cursor, Aider, and other MCP-compatible clients use a similar JSON config. Refer to each client's documentation for the exact config file location and format.

---

## Enterprise Proxy

The enterprise proxy is a **conditional module** — when no enterprise configuration is present, zero enterprise code is loaded, and no `rigorix_enterprise_*` tools appear in the tool list.

When configured, it:
1. Fetches tool schemas from `GET /api/metadata` on startup
2. Registers enterprise tools dynamically (prefix: `rigorix_enterprise_`)
3. Forwards tool calls as HTTP JSON-RPC to the enterprise API
4. Caches schemas with configurable TTL (default: 1 hour)
5. Returns structured diagnostic errors on failure (not raw Rust error dumps)

---

## Development

### Prerequisites

- Rust toolchain (edition 2024)
- `rigorix-engine` built locally (workspace member)

### Build

```bash
cargo build -p rigorix-mcp
```

### Test

```bash
# Unit tests
cargo test -p rigorix-mcp --lib

# Integration tests (includes enterprise proxy E2E, execute-to-audit flow)
cargo test -p rigorix-mcp --test '*'

# All tests
cargo test -p rigorix-mcp
```

### Run Locally

```bash
# stdio mode
cargo run -p rigorix-mcp

# SSE mode
cargo run -p rigorix-mcp -- --sse --bind 127.0.0.1:3001
```

### Project Structure

```
mcp/
├── src/
│   ├── main.rs              # Binary entry point + composition root
│   ├── lib.rs                # Library exports
│   ├── engine_wiring.rs      # Engine dependency construction
│   ├── audit_tools/          # Audit query handlers
│   │   ├── application/      #   Use cases, DTOs
│   │   ├── domain/           #   Entities, formatters, errors
│   │   ├── infrastructure/   #   In-memory audit storage
│   │   └── interfaces/       #   MCP tool handlers
│   ├── enterprise_proxy/     # Enterprise API gateway
│   │   ├── domain/           #   Proxy config, entities, errors
│   │   ├── infrastructure/   #   HTTP client, schema cache
│   │   └── interfaces/       #   MCP tool handlers
│   ├── execution_tools/      # Execution plan tools
│   │   ├── application/      #   Handlers, DTOs, service traits
│   │   ├── domain/           #   Entities, values, errors
│   │   ├── infrastructure/   #   EngineFacade, repository
│   │   └── interfaces/       #   MCP tool schemas
│   ├── mcp_server/           # Core MCP protocol
│   │   └── domain/           #   Server, sessions, registry
│   ├── template_tools/       # Template CRUD tools
│   │   ├── application/      #   Handlers
│   │   ├── domain/           #   Repository interface, entities
│   │   ├── infrastructure/   #   Filesystem repo, TOML/JSON converter
│   │   └── interfaces/       #   MCP tool handlers
│   └── usage_guide/          # Self-documenting guide tool
│       └── interfaces/       #   MCP tool handler
├── tests/                    # Integration tests
│   ├── e2e_enterprise_proxy_test.rs
│   ├── e2e_execute_to_audit_test.rs
│   ├── execution_tools_tdd_test.rs
│   └── stdio_integration_test.rs
├── docs/                     # Architecture runbooks
│   ├── runbook.md
│   ├── runbook-execution-tools.md
│   ├── runbook-mcp-server.md
│   ├── runbook-template-tools.md
│   ├── runbook-enterprise-proxy.md
│   └── publishing.md
└── Cargo.toml
```

---

## Architecture Documentation

The crate is built with the [Guardian Framework](https://github.com/arman-jalili/guardian-framework) — every module has canonical architecture specs in `.pi/architecture/modules/`:

| Module | Doc |
|--------|-----|
| MCP Server | [mcp-server.md](.pi/architecture/modules/mcp-server.md) |
| Execution Tools | [execution-tools.md](.pi/architecture/modules/execution-tools.md) |
| Template Tools | [template-tools.md](.pi/architecture/modules/template-tools.md) |
| Audit Tools | [audit-tools.md](.pi/architecture/modules/audit-tools.md) |
| Enterprise Proxy | [enterprise-proxy.md](.pi/architecture/modules/enterprise-proxy.md) |
| Usage Guide | [usage-guide.md](.pi/architecture/modules/usage-guide.md) |

---

## License

Licensed under either of:

- MIT license ([LICENSE-MIT](https://github.com/arman-jalili/rigorix-oss/blob/main/LICENSE-MIT))
- Apache License, Version 2.0 ([LICENSE-APACHE](https://github.com/arman-jalili/rigorix-oss/blob/main/LICENSE-APACHE))

at your option.
