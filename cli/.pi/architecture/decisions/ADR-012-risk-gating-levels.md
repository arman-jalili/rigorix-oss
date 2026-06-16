# ADR-012: Risk Gating Levels and Policies

**Status:** Accepted
**Date:** 2026-06-16

## Context

Tool execution has varying safety implications. `file_read` is safe; `git_commit` has consequences; `rm -rf /` is destructive. The system must gate tool execution based on risk without blocking the user unnecessarily.

## Decision

**Three risk levels** with configurable policies.

### Risk Level Classification

| Level | Behavior | Examples | TUI Experience |
|-------|----------|---------|----------------|
| Low | Auto-execute | file_read, file_write to known paths, lsp_query, git_read | Shown in log stream, no prompt |
| Medium | User confirmation required | file_patch, git_stage, git_commit, run_command (known commands) | Prompt: "Allow [tool] on [path]? (y/N)" with timeout |
| High | Dry-run first, then confirm | run_command (arbitrary), file_write outside project, git_push | Show diff preview, then ask "Apply? (y/N)" |

### Classification Rules

Classification is determined by:
1. **Tool type**: inherent risk of the operation
2. **Arguments**: e.g., `run_command` with `ls` is Low, with `rm` is High
3. **Target path**: within project (Low) vs outside (High)
4. **User overrides**: `[[tool_aliases]]` in `rigorix.toml` with explicit `risk_level`

```rust
pub enum RiskLevel {
    Low,     // Auto-execute
    Medium,  // Require user confirmation
    High,    // Dry-run then confirm
}
```

### Default Tool Risk Mapping

| Tool | Default Risk | Rationale |
|------|-------------|-----------|
| file_read | Low | Read-only, no side effects |
| file_write | Medium | Can overwrite files |
| file_append | Low | Only appends to existing files |
| file_patch | Medium | Structured modification |
| run_command | Medium (known), High (arbitrary) | Depends on command and arguments |
| lsp_query | Low | Read-only code intelligence |
| git_read | Low | Read-only git operations |
| git_stage | Medium | Prepares files for commit |
| git_commit | Medium | Creates history |
| custom_alias | Per config | User-defined in rigorix.toml |

## Rationale

1. **Three levels** is the minimum needed for meaningful gating without overwhelming the user
2. **Low/Medium/High** is intuitive and maps well to configuration
3. **Configurable overrides** let platform engineers tune policies per project
4. **Medium with timeout** prevents stalled execution (auto-deny after 60s)
5. **High is dry-run first** gives the user a preview before any destructive operation

*Affects: Risk Gating, Enforcement, CLI Boundary*
