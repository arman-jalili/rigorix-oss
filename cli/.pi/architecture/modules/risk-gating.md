# Risk Gating

## Module Status

**Status:** Engine contract frozen — CLI uses as library
**Last reviewed:** 2026-06-16
**Source session:** 71e2b81a-a7a1-48ee-ab8f-56284bbec92d

## Description

Classifies tools/tasks by risk level (Low, Medium, High) and enforces gating policies:
- **Low** (safe): auto-execute
- **Medium** (potentially destructive): require user confirmation
- **High** (dangerous): dry-run or block

Every tool invocation passes through the risk gate before execution. The CLI surfaces confirmation prompts for Medium-risk tools and dry-run output for High-risk tools.

## Components

**Engine dependencies (frozen contracts):**
| Component | Engine Source | Contract |
|-----------|--------------|----------|
| RiskLevel (enum) | `engine/src/risk_gating/domain/risk_level.rs` | `# Contract (Frozen)` |
| RiskConfig | `engine/src/risk_gating/domain/risk_config.rs` | Configurable policy overrides |
| RiskClassifier (trait) | `engine/src/risk_gating/domain/risk_classifier.rs` | Maps tool name → RiskLevel |
| RiskGateService (trait) | `engine/src/risk_gating/application/service.rs` | Gating decision service |
| RiskGatingError | `engine/src/risk_gating/domain/error.rs` | Typed error enum |

## Domain Events

| Event | Description | Triggered By |
|-------|-------------|-------------|
| RiskGateEvaluated | A tool was classified and gated | RiskGateService |

## Ubiquitous Language

| Term | Definition |
|------|-----------|
| RiskLevel | Tool risk classification: Low (safe), Medium (confirm), High (dangerous). |
| RiskClassifier | Service that evaluates a tool name against RiskConfig to determine its RiskLevel. |
| RiskGate | Decision point: auto-execute (Low), confirm (Medium), dry-run/block (High). |

## Dependencies

- Depends on: `engine::risk_gating` (all contracts frozen)
- Depends on: `Configuration` (RiskConfig from rigorix.toml)
- Used by: `Enforcement` (risk level for tool policy selection)
