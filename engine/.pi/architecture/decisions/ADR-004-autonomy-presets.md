# ADR-004: Three Autonomy Presets with Hard Enforcement Caps

**Status:** Accepted
**Date:** 2026-06-13
**Session:** 63c25384-1902-4b72-83bb-257f3f682af5

**Tech Stack:** Rust

## Context

Rigorix must support different levels of execution autonomy while maintaining its core principle of bounded, deterministic behavior. Different use cases require different trade-offs between automation and safety.

## Decision

Provide **three autonomy presets** (Default, Advanced, Aggressive) with hard-capped enforcement limits validated at startup. Users select via CLI `--mode` flag or `rigorix.toml`.

| Limit | Default | Advanced | Aggressive | Absolute Cap |
|-------|---------|----------|------------|-------------|
| Dynamic nodes | 0 | 50 | 200 | 1,000 |
| Execution time | 300s (5m) | 1,800s (30m) | 3,600s (1h) | 7,200s (2h) |
| Tool calls | 100 | 500 | 2,000 | — |
| LLM calls | 5 | 20 | 50 | — |
| LLM tokens | 10,000 | 100,000 | 500,000 | — |
| Parallel tasks | 4 | 8 | 16 | 64 |
| Total retries | 10 | 30 | 100 | — |
| Retries/node | 3 | 3 | 5 | — |

## Alternatives Considered

| Alternative | Pros | Cons | Reason Rejected |
|-------------|------|------|-----------------|
| **Three presets (chosen)** | Clear upgrade path; safe defaults; validated absolute caps | Not infinitely granular | **Chosen** |
| **Single mode (deterministic only)** | Simplest implementation | Advanced use cases blocked entirely | Rejected — limits adoption |
| **Unlimited/trusted mode** | Maximum flexibility | Violates bounded autonomy principle | Rejected — architectural violation |
| **Fully customizable per-field** | Maximum granularity | Support burden; easy to misconfigure and break | Rejected — complexity outweighs benefit |

## Consequences

### Positive
- Default mode is safe for CI/CD (0 dynamic nodes, strict limits)
- Advanced mode enables complex refactoring workflows
- Aggressive mode supports large-scale code generation
- Absolute caps prevent any misuse even with misconfiguration

### Negative
- Three code paths to test (per-mode config validation)
- Users may need custom overrides not covered by the three presets

## Implementation

**Affected Modules:**
- `.pi/architecture/modules/enforcement.md`
- `.pi/architecture/modules/budget-tracking.md`

**Files to Update:**
- `rigorix/src/enforcement.rs` — EnforcementConfig with default_mode/advanced_mode/aggressive_mode constructors
- `rigorix/src/config.rs` — EnforcementPreset enum

---

*Decision date: 2026-06-13*
