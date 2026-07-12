# MCP Server Disaster Recovery Plan

## Overview

**Component:** MCP Server (`rigorix-mcp`)
**Phase:** 0 (In-Memory)
**RTO:** < 5 minutes
**RPO:** Not applicable (no persistent state in Phase 0)

## Architecture State

In Phase 0, the MCP Server is **fully in-memory**:
- **Sessions**: Created on initialize, destroyed on disconnect/timeout
- **Tools**: Registered at startup, static until restart
- **Events**: Emitted but not persisted (future: EventBus with persistence)
- **Config**: CLI flags and environment variables only

No database, no filesystem state, no external dependencies.

## Failure Scenarios

### 1. Process Crash

**Impact:** All active sessions lost. Clients get connection refused.

**Recovery:**
1. Restart the process: `rigorix-mcp` (or with `--sse --bind ...`)
2. Clients auto-reconnect (MCP protocol handles reconnection)
3. Sessions re-initialize via `initialize` handshake
4. Tools re-register at startup

**RTO:** Seconds (process restart)

### 2. OOM (Out of Memory)

**Symptoms:** Process killed by OOM killer, crash log shows `SIGKILL`

**Prevention:**
- Monitor RSS memory usage
- Set `ulimit` limits in production
- In future: streaming pagination for large tool results

**Recovery:** Same as process crash

### 3. Port Conflict (SSE Mode)

**Symptoms:** `Address already in use` on startup

**Recovery:**
1. Kill existing process: `lsof -ti:3001 | xargs kill`
2. Or change bind address: `--bind 127.0.0.1:3002`

### 4. Resource Exhaustion (Too Many Sessions)

**Symptoms:** New sessions rejected with `MaxSessionsReached` error

**Prevention:**
- Default max: 10 sessions (configurable via `ServerConfig::max_sessions`)
- Monitor session count
- Session timeout evicts idle sessions

**Recovery:**
- Wait for idle sessions to timeout
- Or increase max sessions and restart

### 5. Client Misbehavior

**Symptoms:** Client sends malformed JSON-RPC, too many requests, etc.

**Mitigation:**
- Rate limiting per session (future)
- Request timeout (configurable)
- Session-level isolation (one bad client doesn't affect others)

## Backup and Restore

**Phase 0:** No backup needed (no persistent state).

**Phase 1+ (Future):**
- Tool handler results persisted to audit trail
- Template repository on filesystem (`~/.rigorix/templates/`)
- Backup strategy: periodic rsync/copy of template directory
- Restore: copy backup to correct location, restart server

## Failover

**Phase 0:** Single instance. No failover.

**Phase 1+ (Future):**
- Multiple instances behind a load balancer
- Shared nothing architecture
- Each instance registers independently

## Recovery Test Checklist

| Step | Action | Expected Result |
|------|--------|-----------------|
| 1 | Kill rigorix-mcp process | Client reports connection error |
| 2 | Restart rigorix-mcp | Server starts, listens on transport |
| 3 | Client reconnects | Initialize success, tools listed |
| 4 | Execute tool call | Tool returns result successfully |
| 5 | Verify no data loss | All sessions re-created fresh |

## Monitoring Alerts

| Alert | Condition | Action |
|-------|-----------|--------|
| ProcessDown | Process not running | Auto-restart (systemd/supervisor) |
| HighSessionCount | Sessions > 80% of max | Investigate stuck sessions |
| PortNotListening | Health check fails | Restart process |
| HighErrorRate | Tool errors > 5/min | Check tool handlers |
