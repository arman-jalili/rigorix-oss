# Architecture Decision Record: ADR-101

<!--
Canonical Reference: .pi/architecture/decisions/ADR-101-actions-as-thin-adapter.md
Blueprint Source: Rigorix design session (2026-06-20)
-->

## Title

ADR-101: Actions crate must be a thin adapter — no engine logic rebuilt

## Status

- [x] Accepted
- [ ] Deprecated
- [ ] Superseded

## Context

The `rigorix-engine` crate contains all domain logic: planning, execution, validation, hooks, permissions, recovery, quality gates, and policy evaluation. The `actions/` crate needs to expose this functionality as a GitHub Action. Two approaches were considered:

1. **Rebuild**: Implement action-specific versions of engine services inside the actions crate
2. **Adapter**: Wrap engine services via `Arc<dyn Trait>` injection, adding only GitHub-specific I/O

## Decision

**Use the Adapter pattern.** The actions crate:

- Depends on `rigorix-engine` as a library dependency
- Injects engine services (`OrchestratorService`, `ValidationLoopService`, `QualityGateService`) via `Arc<dyn Trait>`
- Adds only GitHub-specific code: input parsing (`INPUT_*` env vars), output formatting (annotations, step summaries), and CI integration (status checks, PR comments)
- Never re-implements planning, execution, validation, or tool logic

## Alternatives Considered

| Alternative | Pros | Cons | Reason Rejected |
|-------------|------|------|-----------------|
| Rebuild engine services in actions | Independent deployment, no engine dependency | Duplication of all domain logic, diverging behavior, maintenance nightmare | Violates DRY, diverges from engine |
| CLI-only (no action crate) | Simpler | No GitHub-native outputs (annotations, status checks, PR comments) | Doesn't meet CI integration requirements |
| FFI/binary invocation | Language-agnostic | Serialization overhead, error handling complexity, no compile-time type safety | Rust-to-Rust is simpler |

## Consequences

**Positive:**
- Zero code duplication between engine and actions
- Engine improvements automatically benefit the action
- Compile-time type safety for all engine service interfaces
- Single source of truth for all domain logic

**Negative:**
- Actions crate version must stay compatible with engine API changes
- Cannot deploy action independently of engine changes
- CI build time includes full engine compilation (mitigated by caching)

## Cross-References

- `engine/.pi/architecture/decisions/ADR-001-architecture-pattern.md` — Clean Architecture pattern
- `engine/.pi/architecture/modules/action-entrypoint.md` — Entrypoint module spec
- `engine/.pi/architecture/modules/action-output.md` — Output formatting module spec

---

*Date: 2026-06-20*
*Session: rigorix-oss design session*
