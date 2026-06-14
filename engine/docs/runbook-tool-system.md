# Runbook: tool-system Module

<!--
Canonical Reference: .pi/architecture/modules/tool-system.md
Last Updated: 2026-06-14
-->

## Overview

The `tool-system` module defines the execution primitives for the task graph.
It provides the `Tool` trait, shared types (`ToolInput`, `ToolResult`, `ToolError`),
the `ToolRegistry`, and concrete tool implementations (FileRead, FileWrite, etc.).

## Components

| Component | Type | Description |
|-----------|------|-------------|
| `Tool` | Domain trait | Core abstraction — all tools implement this |
| `ToolRegistry` | Application service | Registry by tool name with risk-gated execution |
| `ToolError` | Domain enum | 5 error variants with HTTP status mappings |
| `FileReadTool` | Concrete tool | Read file contents (Low risk) |
| `FileWriteTool` | Concrete tool | Atomic write-rename (Medium risk) |
| `FileAppendTool` | Concrete tool | Append to existing files (Medium risk) |
| `FilePatchTool` | Concrete tool | Search/replace patching (Medium risk) |
| `RunCommandTool` | Concrete tool | Shell execution with allowlist (High risk) |
| `LspQueryTool` | Concrete tool | LSP code intelligence queries (Low risk) |
| `GitReadTool` | Concrete tool | Read-only git operations (Low risk) |
| `GitStageTool` | Concrete tool | Stage files in git index (Medium risk) |
| `GitCommitTool` | Concrete tool | Create git commits (High risk) |

## Startup Sequence

1. **Module initialization**: The `tools` module is loaded via `src/lib.rs` at crate startup.
2. **Tool registration**: During orchestrator setup, concrete tool implementations are
   constructed and registered in the `ToolRegistryImpl` using `register_tool()`.
3. **Risk classification**: Each tool receives a default risk level from `risk_mapping.rs`.
   Overrides can be provided via `RiskConfig`.

### Registration Order

```rust
let mut registry = ToolRegistryImpl::new();
registry.register_tool("file-read", Box::new(FileReadTool::new(workspace_root)));
registry.register_tool("file-write", Box::new(FileWriteTool::new(workspace_root)));
registry.register_tool("file-append", Box::new(FileAppendTool::new(workspace_root)));
registry.register_tool("file-patch", Box::new(FilePatchTool::new(workspace_root)));
registry.register_tool("run-command", Box::new(RunCommandTool::new(workspace_root, allowlist)));
registry.register_tool("lsp-query", Box::new(LspQueryTool::new()));
registry.register_tool("git-read", Box::new(GitReadTool::new(repo_root)));
registry.register_tool("git-stage", Box::new(GitStageTool::new(repo_root)));
registry.register_tool("git-commit", Box::new(GitCommitTool::new(repo_root)));
```

## Graceful Shutdown

The tool-system module has no long-lived background tasks or connections.
Shutdown is immediate:
1. Any in-flight tool executions will complete (or timeout) naturally.
2. No explicit cleanup is required — all tools are stateless.
3. The `ToolRegistryImpl`'s internal `RwLock` is dropped when the registry goes out of scope.

## Configuration Reference

| Parameter | Source | Default | Description |
|-----------|--------|---------|-------------|
| `workspace_root` | `ToolSystemConfig` | Current directory | Root for path validation |
| `max_timeout_secs` | `ToolSystemConfig` | 300 | Max execution timeout |
| `max_output_bytes` | `ToolSystemConfig` | 1 MB | Max output capture size |
| `dry_run_high_risk` | `ToolSystemConfig` | true | Dry-run High-risk tools by default |
| `require_medium_confirmation` | `ToolSystemConfig` | true | Require confirmation for Medium-risk |
| `allowlist` | `RunCommandTool` | [] | Allowed command prefixes |

## Common Failure Modes

### Tool Not Found

**Symptom:** `ToolError::NotFound("Tool 'xxx' not found")`
**Cause:** Tool name misspelled or not registered.
**Recovery:** Verify the tool name matches registration. Check `list_tools()`.

### Path Denied

**Symptom:** `ToolError::PathDenied("Path is outside workspace root")`
**Cause:** Tool attempted to access a file outside the configured workspace.
**Recovery:** Use a path within the workspace root, or update `workspace_root`.

### Input Validation Failure

**Symptom:** `ToolError::InvalidInput("Missing required parameter: xxx")`
**Cause:** Required parameter not provided in `ToolInput.params`.
**Recovery:** Check the tool's documentation for required parameters.

### RunCommand Not Allowed

**Symptom:** `ToolError::PathDenied("Command 'xxx' is not in the allowlist")`
**Cause:** Command not in the configured allowlist.
**Recovery:** Add the command prefix to the allowlist, or use an allowed command.

### File Patch Ambiguity

**Symptom:** `ToolError::ExecutionFailed("Search string found N times")`
**Cause:** The search string appears multiple times in the file.
**Recovery:** Use a more specific search string that appears exactly once.

## Dependencies

### Depends On
- **Risk Gating**: `RiskLevel`, `RiskConfig` for gating decisions
- **Configuration**: Workspace root path, path allowlists

### Used By
- **Execution Engine**: ParallelExecutor resolves tools via registry
- **Orchestrator**: Registers tools during build

## Observability

### Logging
- Tool execution events are emitted via `ToolEvent` enum
- Each event carries execution ID, tool name, duration, and risk level
- Errors include full context for debugging

### Metrics
- Tool execution count (per tool name)
- Execution duration (histogram)
- Error rate (per error type)
- Risk level distribution

### Health
- Health check: verify all expected tools are registered
- Dependency check: confirm workspace root is accessible

---

*Last updated: 2026-06-14*
