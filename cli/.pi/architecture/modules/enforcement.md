# Enforcement

## Module Status

**Status:** Engine contract frozen — CLI uses as library
**Last reviewed:** 2026-06-16
**Source session:** 71e2b81a-a7a1-48ee-ab8f-56284bbec92d

## Description

Safety limit enforcement: resource budgets (tokens, tool calls, execution time), tool call policies (Allow/Block/Confirm based on risk level), and execution hard limits. The `ExecutionEnforcer` sits between the executor and tool execution, gating every tool call and tracking resource consumption.

The CLI surfaces budget warnings and enforcement actions in the TUI and log output.

## Components

**CLI-facing:** None — CLI wraps engine contracts directly. No CLI-specific interface files needed.

**Engine dependencies (frozen contracts):**
| Component | Engine Source | Contract |
|-----------|--------------|----------|
| ExecutionEnforcer (trait) | `engine/src/enforcement/application/service.rs` | `# Contract (Frozen)` |
| EnforcementConfig | `engine/src/enforcement/domain/config.rs` | Resource budgets + tool policies |
| ResourceBudget | `engine/src/enforcement/domain/config.rs` | Tracked resource: type, used, soft/hard limit |
| ToolPolicy | `engine/src/enforcement/domain/config.rs` | Per-tool policy: Allow, Block, Confirm, DryRun |
| EnforcementError | `engine/src/enforcement/domain/error.rs` | Typed error enum |

## Domain Events

| Event | Description | Triggered By |
|-------|-------------|-------------|
| ToolExecuted | A tool was evaluated (allowed, blocked, or confirmed) | ExecutionEnforcer |
| ToolBlocked | A tool was blocked by policy | ExecutionEnforcer |
| BudgetWarning | A resource budget threshold was hit | ExecutionEnforcer |

## Ubiquitous Language

| Term | Definition |
|------|-----------|
| ExecutionEnforcer | Safety gate that checks budgets, risk levels, and tool policies before every tool call. |
| EnforcementConfig | Configuration for resource budgets and tool call policies. |
| ResourceBudget | Tracked resource with soft warning limit and hard enforcement limit. |
| ToolPolicy | Per-tool or per-risk-level policy: Allow, Block, Confirm, DryRun. |

## Dependencies

- Depends on: `engine::enforcement` (all contracts frozen)
- Depends on: `Configuration` (EnforcementPreset from Config)
- Depends on: `Budget Tracking` (LLM budget for plan phase)
- Depends on: `Risk Gating` (risk level for tool policy selection)
- Used by: `Execution Engine` (gates every tool call)
