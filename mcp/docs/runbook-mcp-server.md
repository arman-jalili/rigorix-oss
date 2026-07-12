# MCP Server Runbook

## Overview

The MCP Server (`rigorix-mcp`) is the Model Context Protocol gateway for Rigorix.
It bridges AI coding assistants (Claude Code, Cursor, etc.) with the Rigorix engine.

## Architecture

```
┌──────────────┐     ┌──────────────┐     ┌──────────────┐
│ MCP Client   │◄───►│ MCP Server   │◄───►│ Tool Handlers│
│ (AI Tool)    │     │ (rigorix-mcp)│     │ (Rigorix)   │
└──────────────┘     └──────────────┘     └──────────────┘
                           │
                    ┌──────┴──────┐
                    │ In-Memory   │
                    │ Storage     │
                    └─────────────┘
```

## Startup

### Prerequisites
- Rust toolchain (edition 2024)
- Build: `cargo build -p rigorix-mcp`

### Startup Sequence
1. Binary starts, parses CLI args (`--sse`/`--bind` for SSE mode)
2. Creates in-memory repositories
3. Creates McpServer aggregate with `ServerConfig`
4. Calls `McpServer::start()` → transitions to `Initializing`
5. Opens transport channel (stdio or SSE)
6. Calls `McpServer::on_transport_opened()` → transitions to `Running`
7. Server is ready for client connections

### stdio Mode (Default)
```
rigorix-mcp
```
- Reads JSON-RPC messages from stdin (newline-delimited)
- Writes responses to stdout
- Used by: Claude Code, Aider

### SSE Mode
```
rigorix-mcp --sse --bind 127.0.0.1:3001
```
- Listens on configured address
- SSE endpoint for streaming responses
- Used by: Claude Desktop, Cursor

## Shutdown

### Graceful Shutdown
1. Send SIGINT (Ctrl+C) or SIGTERM
2. Server calls `McpServer::shutdown()` → transitions to `ShuttingDown`
3. Drains active requests (timeout: 30s max)
4. Closes transport channel
5. Drops sessions
6. Transitions to `Stopped`

### Force Shutdown
If graceful shutdown takes >30s, send SIGKILL.
This may leave partial state (acceptable for in-memory Phase 0).

## Common Failure Modes

| Failure | Symptom | Recovery |
|---------|---------|----------|
| Transport error | Connection drops | Client reconnects; server continues listening |
| Invalid JSON-RPC | Parse error returned | Client retries with valid message |
| Tool not found | MethodNotFound error | Check tool registration |
| Session timeout | No activity >5min | Session evicted; client re-initializes |
| Port conflict | Bind error (SSE mode) | Change bind address |
| OOM | Server crash | Restart with higher memory limit |

## Configuration Reference

| Variable | CLI Flag | Default | Description |
|----------|----------|---------|-------------|
| `MCP_TRANSPORT` | `--sse` | stdio | Transport mode |
| `MCP_BIND` | `--bind` | 127.0.0.1:3001 | SSE bind address |
| `MCP_MAX_SESSIONS` | — | 10 | Max concurrent sessions |
| `MCP_SESSION_TIMEOUT` | — | 300 | Session timeout (seconds) |

## Health Check

In SSE mode, the server exposes a health endpoint:
```
GET /health → { "status": "ok", "uptime": "1234s", "sessions": 2, "tools": 10 }
```

## Logging

Structured JSON logging via `tracing` crate:
```
{"level":"INFO","message":"Session started","session_id":"abc123","client":"claude-code"}
{"level":"WARN","message":"Tool not found","tool_name":"rigorix_unknown"}
{"level":"ERROR","message":"Transport error","error":"connection reset"}
```

## Metrics (Phase 1+)

| Metric | Type | Description |
|--------|------|-------------|
| mcp_sessions_active | Gauge | Current active sessions |
| mcp_tools_registered | Gauge | Registered tool count |
| mcp_tool_calls_total | Counter | Total tool calls |
| mcp_tool_call_duration | Histogram | Tool call latency |
