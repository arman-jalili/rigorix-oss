# Disaster Recovery Plan: hooks Module

<!--
Canonical Reference: .pi/architecture/modules/hooks.md
Last Updated: 2026-06-19
-->

## Scope

This DR plan covers the `hooks` module — external script-based interception
points that run as child processes around every tool execution. Hooks are
stateless: configuration is loaded from `.rigorix/hooks.toml` at startup, and
each hook execution is an independent child process.

## RTO/RPO Targets

| Metric | Target | Rationale |
|--------|--------|-----------|
| RTO (Recovery Time Objective) | < 30 seconds | Module is stateless — re-create `HookRunner` with config |
| RPO (Recovery Point Objective) | N/A (stateless) | Hook configuration is stored in filesystem (`.rigorix/hooks.toml`) |

## Backup Strategy

### Configuration

Hook configuration is stored in the workspace's `.rigorix/hooks.toml` file.
This file should be version-controlled (committed to the project repository).

| Data | Backup Strategy | Frequency | Retention |
|------|----------------|-----------|-----------|
| `hooks.toml` config | Version control (git) | On change | Full git history |

### No State to Back Up

The hooks module is **stateless**:
- Hook commands are executed as child processes — no internal state is modified
- Hook results (`HookRunResult`) are ephemeral — consumed immediately by the execution engine
- No database, no queue, no persistent storage is managed by this module

## Restore Procedure

### Scenario: Hook Configuration Lost or Corrupted

1. **Detect**: Engine logs `HookError::CommandNotFound` or configuration parse error
2. **Isolate**: Verify `.rigorix/hooks.toml` exists and is valid TOML
3. **Restore**: 
   ```bash
   # Restore from version control
   git checkout HEAD -- .rigorix/hooks.toml
   ```
4. **Verify**:
   ```bash
   # Validate hook config
   bash engine/.pi/scripts/ci/check_hooks_contracts.sh
   ```
5. **Recover**: Restart the engine or trigger config reload

### Scenario: Hook Binary Missing

1. **Detect**: `HookError::CommandNotFound` for hook command
2. **Isolate**: Check if binary exists in PATH
   ```bash
   which rigorix-hook-validate-path
   ```
3. **Restore**: Install missing hook binary:
   ```bash
   cargo install rigorix-hooks
   ```
4. **Verify**: Test hook execution directly:
   ```bash
   echo '{"event":"pre_tool_use","tool_name":"ls","tool_input":{},"session_id":"test","workspace_root":"."}' \
     | rigorix-hook-validate-path
   ```
5. **Recover**: Restart engine or trigger config reload

### Scenario: All Hooks Failing

1. **Stop**: Create `HookAbortSignal` to cancel running executions
2. **Remove**: Temporarily clear hook config:
   ```toml
   [hooks]
   pre_tool_use = []
   post_tool_use = []
   post_tool_use_failure = []
   ```
3. **Diagnose**: Test each hook in isolation
4. **Restore**: Re-add verified hooks one at a time

## Failover Plan

### Single Instance (Default Deployment)

The hooks module runs in-process within the engine. Failover follows the engine's
failover plan:

1. Engine process fails → OS restarts (systemd/supervisor)
2. Engine re-initializes → `HookRunner` created fresh from config
3. Hook binaries remain on filesystem — no data loss

### High-Availability Deployment

Not currently supported — hooks run in-process. If HA is required:

1. Deploy engine behind a load balancer (multiple replicas)
2. Each replica loads the same `.rigorix/hooks.toml` from shared config
3. Hook binaries must be installed on every replica node
4. File system hooks must use shared storage (NFS) or be containerized

## Disaster Scenarios

### Scenario 1: Malicious Hook Script

| Aspect | Detail |
|--------|--------|
| **Impact** | Hook could modify files, exfiltrate data, or block legitimate operations |
| **Detection** | Unexpected tool blocks, modified tool inputs, file system changes |
| **Containment** | Kill engine process; remove malicious hook from config |
| **Recovery** | Audit hook script; restore from git history; add security validation |
| **Prevention** | Hooks run from `.rigorix/hooks/` directory only; validate hook paths |

### Scenario 2: Hook Blocks All Tool Execution

| Aspect | Detail |
|--------|--------|
| **Impact** | All tool executions denied — engine cannot function |
| **Detection** | Every tool returns `PermissionOutcome::Deny` |
| **Containment** | Clear `pre_tool_use` config and restart engine |
| **Recovery** | Diagnose hook decision logic; fix deny condition; re-add hooks |
| **Prevention** | Test hooks in CI; add integration tests for deny/allow decisions |

### Scenario 3: Hook Corrupts Tool Input

| Aspect | Detail |
|--------|--------|
| **Impact** | Tools execute with modified input — could cause data corruption |
| **Detection** | Unexpected tool behavior; audit trail shows `updated_input` |
| **Containment** | Disable hooks that use `Modify` decision; restart engine |
| **Recovery** | Review hook `Modify` logic; validate input transformation |
| **Prevention** | Immutable audit trail of original vs. modified input |

## Testing DR Readiness

| Test | Frequency | Procedure |
|------|-----------|-----------|
| Config load failure | Quarterly | Corrupt `.rigorix/hooks.toml` → verify engine degrades gracefully |
| Hook binary missing | Quarterly | Remove hook binary → verify `CommandNotFound` handled |
| Hook timeout | Quarterly | Set `timeout_secs: 1` with slow hook → verify timeout handling |
| Hook abort signal | Quarterly | Set abort before execution → verify cancelled result |
| All hooks denied | Quarterly | Configure deny-all hook → verify execution blocked gracefully |

## DR Kit Contents

| Item | Location | Purpose |
|------|----------|---------|
| Last known good `.rigorix/hooks.toml` | Version control | Emergency rollback of hook config |
| Hook binary installation script | Repository | Re-install hook binaries |
| DR test scripts | `.pi/scripts/dr/` | Automated DR readiness tests |
| Architecture docs | `.pi/architecture/modules/hooks.md` | System understanding |

## Related Documents

- [Runbook: hooks](runbook-hooks.md)
- [Architecture: hooks](../.pi/architecture/modules/hooks.md)
- [Contract Freeze: hooks](../.pi/issues/issue-contract-freeze.md)
