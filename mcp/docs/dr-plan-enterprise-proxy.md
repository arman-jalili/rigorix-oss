# Disaster Recovery Plan: enterprise-proxy

## Overview

Enterprise-proxy is a thin HTTP proxy layer between the MCP server and the Rigorix
Enterprise API. It has no local persistent state — all data is fetched dynamically
from the enterprise API. Recovery is primarily about restoring connectivity and
configuration.

## Recovery Objectives

| Metric | Target |
|--------|--------|
| Recovery Time Objective (RTO) | 5 minutes (configuration restore) |
| Recovery Point Objective (RPO) | N/A (no persistent state) |
| Maximum Tolerable Downtime | 15 minutes |
| Data Loss Tolerance | None (stateless proxy) |

## Failure Scenarios

### Scenario 1: Configuration Corruption

**Symptoms**: Enterprise proxy fails to initialize; `is_enabled()` returns `false`.

**Impact**: All `rigorix_enterprise_*` tools are unavailable.

**Recovery**:
1. Verify configuration file integrity
2. Restore configuration from backup or version control
3. Restart MCP server
4. Verify: `is_enabled() == true` and `health_check()` succeeds

### Scenario 2: API Key Rotation

**Symptoms**: Authentication failures (401/403) on enterprise tool calls.

**Impact**: Enterprise functionality fully degraded.

**Recovery**:
1. Obtain new API key from enterprise administrator
2. Update `enterprise.api_key` in configuration
3. Restart MCP server
4. Verify: Schema fetch succeeds and tool calls work

### Scenario 3: Enterprise API Outage

**Symptoms**: Tool calls return timeout or transport errors.

**Impact**: Enterprise functionality unavailable for duration of outage.

**Recovery**:
1. Confirm API status with enterprise administrator
2. No MCP server changes needed — proxy automatically recovers when API is restored
3. Schema cache (TTL: 1 hour) may serve stale data but tool calls fail immediately
4. If prolonged outage expected, disable enterprise proxy via config removal

### Scenario 4: MCP Server Migration

**Symptoms**: Enterprise proxy running on new host cannot reach enterprise API.

**Impact**: Full enterprise functionality loss on migrated host.

**Recovery**:
1. Copy enterprise configuration to new host
2. Verify network connectivity: `curl -I ${API_URL}/api/health`
3. Ensure outbound HTTPS is permitted in firewall
4. Restart MCP server
5. Verify: `health_check()` returns healthy status

## Backup Strategy

| Asset | Method | Frequency | Retention |
|-------|--------|-----------|-----------|
| Enterprise configuration | Version-controlled config file | On change | Indefinite (git history) |
| API key | Secure secrets manager | On rotation | Per secrets manager policy |
| Schema cache (optional) | In-memory only, regenerated on restart | N/A | N/A |

## Restore Procedure

### Full Restore

```bash
# 1. Restore configuration
cp /backup/enterprise-config.toml /etc/rigorix/enterprise-config.toml

# 2. Restore API key from secrets manager
vault read -field=api_key secret/rigorix/enterprise > /etc/rigorix/api_key
chmod 600 /etc/rigorix/api_key

# 3. Verify connectivity
curl -s -H "Authorization: Bearer $(cat /etc/rigorix/api_key)" \
  https://enterprise.example.com/api/health

# 4. Restart MCP server
systemctl restart rigorix-mcp

# 5. Verify recovery
# Check logs for successful initialization
journalctl -u rigorix-mcp --since "5 minutes ago" | grep "enterprise_proxy"
```

## Failover Plan

Enterprise-proxy has no active/passive failover built in. For high availability:

### Option 1: Multiple MCP Servers with Load Balancer
- Deploy two MCP server instances behind a TCP load balancer
- Each instance independently connects to the same enterprise API
- No session affinity needed (stateless proxy)
- Failover is automatic via load balancer health checks

### Option 2: Enterprise API Multi-Region
- Enterprise API deployed across multiple regions
- Configure `enterprise.api_url` with a region-aware DNS
- Failover handled at DNS level

## Testing the Plan

| Test | Frequency | Success Criteria |
|------|-----------|-----------------|
| Configuration restore | Monthly | Full init within 5 min |
| API key rotation | Quarterly | New key accepted, old key rejected |
| Network partition | Quarterly | Proxy reports clear errors, recovers on reconnect |
| Restart recovery | Every deploy | Proxy initializes and tools register within 10s |
