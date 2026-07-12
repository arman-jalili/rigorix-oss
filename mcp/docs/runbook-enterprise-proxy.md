# Runbook: enterprise-proxy

## Overview

Enterprise-proxy forwards `rigorix_enterprise_*` tool calls to the Rigorix Enterprise
API via HTTP JSON-RPC. It dynamically discovers available enterprise tools during
initialization and caches their schemas.

This is a **conditional module** — when no enterprise configuration is present, zero
enterprise code is loaded, and no `rigorix_enterprise_*` tools appear in the MCP
tool list.

## Startup Sequence

1. **Configuration required**: `enterprise.api_url` and `enterprise.api_key` must be
   set in the MCP server configuration
2. **Dependencies available**: MCP Server must be running (provides ToolRegistry for
   dynamic tool registration)
3. **Initialization**: On startup, EnterpriseProxy fetches tool schemas from
   `GET /api/metadata` and populates the SchemaCache
4. **Tool registration**: Enterprise tools are dynamically registered in ToolRegistry
   after successful initialization

### Startup Order

```
MCP Server → EnterpriseProxy (if configured) → Schema fetch → Tool registration
```

### Conditional Path

```
No enterprise config → EnterpriseProxy disabled → No enterprise tools registered
Enterprise config present → EnterpriseProxy enabled → Schemas fetched → Tools registered
```

## Configuration

| Parameter | Default | Description |
|-----------|---------|-------------|
| `enterprise.api_url` | — (required) | Base URL of the Rigorix Enterprise API (must be HTTPS) |
| `enterprise.api_key` | — (required) | Enterprise API key (stored as Secret — redacted in logs) |
| `enterprise.timeout_secs` | 30 | Request timeout in seconds |
| `enterprise.tls_verify` | true | Whether to verify TLS certificates |
| `enterprise.max_retries` | 3 | Maximum retry attempts on transient errors |
| `enterprise.schema_ttl_secs` | 3600 | Schema cache TTL in seconds |

## Graceful Shutdown

Enterprise-proxy uses no background tasks or persistent connections that require
explicit cleanup. HTTP connections are managed by reqwest's connection pool which
drains automatically on shutdown.

## Health Check

Enterprise-proxy health is verified by:
1. `EnterpriseProxy::is_enabled()` returns `true` (configuration present and init completed)
2. `EnterpriseProxy::health_check()` succeeds (enterprise API responds)
3. Schema cache is populated (`SchemaCache::is_populated()` returns `true`)
4. At least one enterprise tool is registered in ToolRegistry

## Common Failure Modes

### Enterprise Proxy Not Enabled

**Symptom**: `rigorix_enterprise_*` tools not listed in MCP tools/list

**Cause**: No enterprise configuration provided.

**Resolution**: Set `enterprise.api_url` and `enterprise.api_key` in configuration
and restart the MCP server.

### Authentication Failure

**Symptom**: Enterprise tool calls return 401/403 errors

**Cause**: API key is invalid, expired, or revoked.

**Resolution**: Verify the API key with your enterprise administrator. Update
`enterprise.api_key` in configuration and restart.

### Enterprise API Unreachable

**Symptom**: Tool calls timeout or return transport errors

**Cause**: Network issue between MCP server and enterprise API.

**Resolution**:
1. Check network connectivity: `curl -I ${enterprise.api_url}/api/health`
2. Verify firewall rules allow outbound HTTPS to the enterprise API
3. Check enterprise API server status
4. Increase `enterprise.timeout_secs` if latency is high

### Schema Fetch Failed

**Symptom**: Enterprise tools fail to register on startup

**Cause**: `GET /api/metadata` endpoint is unreachable or returns errors.

**Resolution**: The proxy enters `Degraded` state. Enterprise tool calls will retry
schema fetch on the next tool call. If the metadata endpoint is restored, the proxy
recovers automatically.

## Debugging

### Enable debug logging

```bash
RUST_LOG=rigorix_mcp::enterprise_proxy=debug cargo run
```

### Test enterprise API connectivity

```bash
curl -s -H "Authorization: Bearer ${API_KEY}" ${API_URL}/api/health
```

### Check registered enterprise tools

```bash
# Through MCP protocol
echo '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}' | nc localhost 8080
```

## Alerts

| Condition | Severity | Action |
|-----------|----------|--------|
| Enterprise proxy fails to initialize | Warning | Check enterprise configuration |
| Enterprise API returns 401/403 | Critical | Rotate API key |
| Enterprise API timeout rate > 10% | Warning | Check network/API latency |
| Schema cache empty for > 5 min | Warning | Check metadata endpoint |
| Tool call error rate > 5% | Critical | Escalate to enterprise admin |
