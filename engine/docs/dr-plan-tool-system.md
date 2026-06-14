# Disaster Recovery Plan: tool-system Module

<!--
Canonical Reference: .pi/architecture/modules/tool-system.md
Last Updated: 2026-06-14
-->

## Scope

This DR plan covers the `tool-system` module — the execution primitives for the
task graph (Tool trait, ToolRegistry, and concrete tool implementations). Since
tools are stateless and defined in code, the primary risks are configuration
misalignment and path security violations.

## RTO/RPO Targets

| Metric | Target | Rationale |
|--------|--------|-----------|
| RTO (Recovery Time Objective) | < 1 minute | Tools are stateless; restart and re-register |
| RPO (Recovery Point Objective) | N/A | No persistent state to recover |

## Backup Strategy

No persistent backup is needed for the tool-system module because:

- **Tool implementations**: Compiled into the binary (backed by source control)
- **Tool configurations**: Loaded from `ToolSystemConfig` (app config)
- **Execution history**: Stored by the `ToolRepository` interface (opt-in)

| Data | Backup Method | Frequency | Location |
|------|--------------|-----------|----------|
| Tool source code | Git | Every commit | GitHub |
| Tool configuration | Config file | Every deploy | `rigorix.toml` |
| Workspace data | Git + filesystem | Continuous | Project directory |

## Restore Procedure

### Scenario 1: Corrupted Tool Configuration

```bash
# 1. Restore default tool configuration
# Tools are stateless — restart with clean config
cargo run -- --config rigorix.toml

# 2. Verify all tools are registered
curl http://localhost:8080/api/v1/tools

# 3. Verify critical tool works
curl -X POST http://localhost:8080/api/v1/tools/execute \
  -H 'Content-Type: application/json' \
  -d '{"tool_name": "file-read", "params": {"path": "README.md"}}'
```

### Scenario 2: Workspace Root Misconfiguration

```bash
# 1. Verify current workspace root
cat rigorix.toml | grep workspace_root

# 2. Correct the path
sed -i 's|workspace_root = ".*"|workspace_root = "/correct/path"|' rigorix.toml

# 3. Restart and verify
cargo run
```

### Scenario 3: RunCommand Allowlist Breach

```bash
# 1. Review current allowlist
grep allowlist rigorix.toml

# 2. Tighten allowlist entries
# Remove dangerous entries, add only needed command prefixes

# 3. Restart to apply
cargo run
```

## Failover Plan

The tool-system module has no active/standby failover requirements:

- **No shared state**: Each instance manages its own tool registry
- **No distributed coordination**: Tools execute independently
- **No leader election**: Not applicable

### Multi-Instance Considerations

If running multiple instances:
- Each instance registers all 9 tools independently
- No synchronization needed between instances
- Load balancer can route to any instance

## Testing the DR Plan

| Test | Frequency | Procedure |
|------|-----------|-----------|
| Tool registration | Every deploy | Run `check_tool-system_contracts.sh` |
| Path security | Every deploy | Run `validate-security.sh` |
| Full recovery | Monthly | Follow restore procedure above |

## Incident Response

### Severity Levels

| Level | Definition | Response Time |
|-------|------------|---------------|
| SEV1 | All tool executions fail | 15 minutes |
| SEV2 | Single tool fails | 1 hour |
| SEV3 | Configuration issue | 4 hours |
| SEV4 | Non-critical warning | Next business day |

### SEV1 Response

1. **Detect**: All tool executions return errors
2. **Triage**: Check if workspace root exists and is accessible
3. **Mitigate**: Verify `workspace_root` in config, restart
4. **Resolve**: Confirm tool execution works via API
5. **Post-mortem**: Root cause analysis within 5 business days

## Configuration Reference

```toml
[tool_system]
# Root directory for path validation
workspace_root = "/path/to/workspace"
# Maximum execution timeout in seconds
max_timeout_secs = 300
# Maximum output size in bytes
max_output_bytes = 1048576
# Dry-run High-risk tools by default
dry_run_high_risk = true
# Require confirmation for Medium-risk tools
require_medium_confirmation = true
```

---

*Last updated: 2026-06-14*
