# Usage Guide

## Module Status

**Status:** Implemented
**Last reviewed:** 2026-07-13
**Last updated:** 2026-07-13

## Description

Self-documenting MCP tool (`rigorix_get_usage_guide`) that returns structured context about valid action types, intent formats, workflow patterns, plan JSON structure, and template file format. AI assistants call this at runtime to understand how to use the rigorix tool system correctly without requiring prior knowledge.

## Architecture

This is a thin module with a single interface-layer handler — no domain or application layers needed since the guide content is static and has no dependencies on other services.

| Layer | Responsibility | Path |
|-------|---------------|------|
| **Interfaces** | MCP tool schema, handler, and guide content builder | `src/usage-guide/interfaces/` |

## Related ADRs

| ADR | Relevance |
|-----|-----------|
| [ADR-001](./decisions/ADR-001-architecture-pattern.md) | Usage Guide is a utility tool within the MCP bounded context |
| [ADR-004](./decisions/ADR-004-mcp-protocol-design.md) | Defines MCP tool registration — usage guide is registered like all other tools |

## Components

### UsageGuideHandler (Interface)

Handles `rigorix_get_usage_guide` tool calls: returns static, structured usage context. No input parameters required.

**Implementation File:** `src/usage-guide/interfaces/mcp/mod.rs`

**Key behaviors:**
- Returns JSON content with workflow patterns (`rigorix_plan` → `rigorix_run` → `rigorix_read_audit`)
- Documents 9 valid action types (`file_read`, `file_write`, `file_append`, `edit_file`, `file_patch`, `run_command`, `git_read`, `git_stage`, `git_commit`)
- Provides example plan JSON structure with required fields
- Documents TOML template file format with example
- Lists all available MCP tools grouped by category (auth, execution, template, audit, enterprise, guide)
- Documents the approval workflow: `rigorix_approve_execution` with `approver_id`/`authority`/`decision_context` (approval binding) and the `rigorix_auth_*` tools (identity attestation)

## Data Flow

```
MCP Client → tools/call { name: "rigorix_get_usage_guide" }
                    │
                    ▼
            handle_get_usage_guide()
                    │
                    ▼
            build_guide() → static JSON
                    │
                    ▼
            ToolCallResult { content: [{ type: "text", text: <guide> }] }
```

## API Endpoints (MCP Tool Schema)

| Method | Path (tool name) | Handler | Input | Output | Auth |
|--------|-----------------|---------|-------|--------|------|
| `rigorix_get_usage_guide` | `tools/call` | handle_get_usage_guide | `{}` | Structured guide JSON with workflow, action types, plan structure, template format, and tool listing | Session-bound |

## Ubiquitous Language

| Term | Definition |
|------|-----------|
| **Usage Guide** | Self-documenting JSON payload describing rigorix tool system capabilities and patterns |
| **Action Type** | One of 9 valid tool operations (file_read, file_write, file_append, etc.) usable in template steps |
| **Workflow** | Recommended sequence: create template → rigorix_plan → rigorix_run → rigorix_read_audit |

## Dependencies

### Depends On
- **MCP Server**: Tool registration via ToolRegistry

### Used By
- **AI Assistants**: Claude and other MCP clients call this at runtime for self-documentation
- None (no internal module dependencies)

## Testing Requirements

| Test Type | Coverage Target | Scenarios |
|-----------|----------------|-----------|
| Unit | Basic | Handler returns valid JSON, guide contains all sections |

**Key Test Scenarios:**
1. Handler returns `ToolCallResult` with text content
2. Guide JSON contains all required sections (workflow, action_types, plan_json_structure, template_file_format, available_tools)
3. All 9 action types documented
4. All tool categories listed (execution, template, audit, enterprise, guide)

---

Last updated: 2026-07-13
*Module version: 1.0.0*

---

**Status:** Implemented
**Last verified:** 2026-07-13
**Module version:** 1.0.0
