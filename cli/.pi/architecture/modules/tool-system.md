# Tool System

## Module Status

**Status:** Engine contract frozen — CLI uses as library
**Last reviewed:** 2026-06-16
**Source session:** 71e2b81a-a7a1-48ee-ab8f-56284bbec92d

## Description

Execution primitives: Tool trait for tool invocations, ToolRegistry for lookup, RiskLevel mapping. Bridges the engine executor to actual side-effect operations (file read/write, command execution, git ops, LSP queries).

The CLI may define custom tool aliases via `rigorix.toml` `[[tool_aliases]]` sections for user-defined script wrappers (deferred plugin system — see Open Questions in exploration).

## Components

**CLI-facing:** None — CLI wraps engine contracts directly. No CLI-specific interface files needed.

**Engine dependencies (frozen contracts):**
| Component | Engine Source | Contract |
|-----------|--------------|----------|
| Tool (trait) | `engine/src/tools/domain/tool.rs` | `# Contract (Frozen)` |
| ToolRegistry | `engine/src/tools/` | Maps tool names to Tool implementations |
| ToolInput | `engine/src/tools/domain/` | Tool input schema |
| ToolResult | `engine/src/tools/domain/` | Tool execution result |
| ToolError | `engine/src/tools/domain/error.rs` | Typed error enum |

## Domain Events

| Event | Description | Triggered By |
|-------|-------------|-------------|
| (uses Enforcement's ToolExecuted event) | — | — |

## Ubiquitous Language

| Term | Definition |
|------|-----------|
| Tool | Trait representing an executable action with name, input schema, and risk level. |
| ToolRegistry | Registry mapping tool names to Tool implementations, built at orchestrator startup. |
| ToolAlias | Configurable named command wrapper in rigorix.toml (v1 plugin system substitute). |

## Dependencies

- Depends on: `engine::tools` (all contracts frozen)
- Depends on: `Risk Gating` (risk level per tool)
- Used by: `Execution Engine` (resolves tool via registry for node execution)
