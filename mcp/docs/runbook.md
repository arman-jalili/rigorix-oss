# Runbook: rigorix-mcp

## Module Runbooks

- [execution-tools](runbook-execution-tools.md) — Plan execution, validation, enforcement checks
- [mcp-server](runbook-mcp-server.md) — MCP protocol server, session management, tool routing

## Incident Procedures

### Service Unavailable
1. Check which module is failing
2. Review the module-specific runbook for recovery steps
3. If MCP connections are failing, restart the MCP Server
4. If tool calls are failing, verify rigorix-engine health
5. Escalate if unresolved after 5 minutes

### Data Loss
- No persistent state in execution-tools — all results are in-memory
- MCP Server session data is ephemeral — reconnect clients on restart
- For audit trail recovery, query rigorix-engine directly
