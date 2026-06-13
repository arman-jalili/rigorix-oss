# ADR-007: Risk Gating Model — Low / Medium / High

**Status:** Accepted
**Date:** 2026-06-13
**Session:** 63c25384-1902-4b72-83bb-257f3f682af5

**Tech Stack:** Rust

## Context

Tools in Rigorix have varying levels of destructive potential: reading a file is harmless, writing a file changes state, running a shell command or committing to git is irreversible. The system must gate tool execution appropriately without blocking safe operations.

## Decision

Classify every tool into one of three risk levels with corresponding gating policies, determined by a dedicated RiskClassifier component.

| Risk Level | Gate Policy | Example Tools |
|------------|-------------|---------------|
| **Low** | Auto-execute — no gate | FileRead, LspQuery, GitRead |
| **Medium** | User confirmation required | FileWrite, FileAppend, FilePatch, GitStage |
| **High** | Dry-run by default (preview, no side effects) | RunCommand, GitCommit |

Risk is determined by tool name. The RiskClassifier maps known tool names to levels. Overrides are configurable via `rigorix.toml` `[tools.risk.tool_overrides]`.

## Alternatives Considered

| Alternative | Pros | Cons | Reason Rejected |
|-------------|------|------|-----------------|
| **Three levels with classifier (chosen)** | Clear semantics; configurable overrides; explicit gating per tool | Requires keeping tool-risk mapping in sync | **Chosen** |
| **Binary (safe/dangerous)** | Simpler classification | Too coarse — treats file writes same as shell execution | Rejected — insufficient granularity |
| **Per-tool risk in config** | Fully customizable | Users must define risk for every tool; no safe defaults | Rejected — bad UX, unsafe by default |
| **LLM-determined risk** | Adaptive, context-aware | Non-deterministic; violates auditability principle | Rejected — architectural violation |

## Consequences

### Positive
- Safe-by-default: read operations are frictionless
- Medium risk creates a safe confirmation point before state mutation
- High risk dry-run prevents accidental destructive operations
- Configurable overrides give power users control
- RiskClassifier provides a single, auditable source of truth

### Negative
- Confirmation flow for Medium risk slows rapid iteration
- Dry-run for High risk requires explicit opt-in to execute
- Risk classification must be kept in sync with new tools

## Implementation

**Affected Modules:**
- `.pi/architecture/modules/risk-gating.md`
- `.pi/architecture/modules/tool-system.md`

**Files to Update:**
- `rigorix/src/core.rs` — RiskLevel enum
- `rigorix/src/config.rs` — RiskConfig struct

---

*Decision date: 2026-06-13*
