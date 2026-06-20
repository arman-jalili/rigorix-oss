# Architecture Decision Record: ADR-102

<!--
Canonical Reference: .pi/architecture/decisions/ADR-102-github-event-routing.md
Blueprint Source: Rigorix design session (2026-06-20)
-->

## Title

ADR-102: GitHub events map to engine modes via a stateless routing table

## Status

- [x] Accepted
- [ ] Deprecated
- [ ] Superseded

## Context

GitHub Actions supports multiple trigger events: `workflow_dispatch`, `issue_comment`, `pull_request`, `push`, `schedule`. Each must map to engine execution modes (Run, Plan, Validate, Status). The routing must be:

- **Deterministic**: same event always produces same engine mode
- **Auditable**: routing decisions are logged and traceable
- **Extensible**: new triggers can be added without changing the router core

## Decision

**Use a stateless routing table.** The `ActionRouter` contains a static mapping:

```
workflow_dispatch with mode:run       → OrchestratorService::run()
workflow_dispatch with mode:validate  → ValidationLoopService::validate()
workflow_dispatch with mode:plan      → OrchestratorService::plan_only()
issue_comment with /rigorix run       → OrchestratorService::run()
issue_comment with /rigorix validate  → ValidationLoopService::validate()
issue_comment with /rigorix plan      → OrchestratorService::plan_only()
issue_comment with /rigorix status    → OrchestratorService::status()
pull_request opened/synchronize       → ValidationLoopService::validate()
push                                  → OrchestratorService::status()
```

The router is stateless — no session state, no event buffering. Each invocation is independent. State lives in the engine's `Orchestrator` and `StateManager`.

## Alternatives Considered

| Alternative | Pros | Cons | Reason Rejected |
|-------------|------|------|-----------------|
| Stateful router with event queue | Supports event correlation across runs | Adds complexity, state management, potential for queue buildup | GitHub Actions are inherently stateless per run |
| Dynamic routing (LLM-decided) | Flexible | Non-deterministic, hard to audit, potential security risk | Violates repeatability principle |
| Hardcoded if-else in main.rs | Simple | Not testable, not extensible | Doesn't meet architecture quality bar |

## Consequences

**Positive:**
- Routing is testable: each event/mode pair is a unit test
- Extensible: new events added by extending the enum and routing table
- Auditable: every routing decision is logged via tracing

**Negative:**
- Must maintain the routing table in sync with engine service API changes
- `PullRequest` event requires GitHub token for PR comment posting (handled by ci-integration module)

## Cross-References

- `engine/.pi/architecture/decisions/ADR-001-architecture-pattern.md` — Clean Architecture pattern
- `engine/.pi/architecture/decisions/ADR-004-autonomy-presets.md` — Validate mode autonomy
- `actions/.pi/architecture/modules/action-entrypoint.md` — Entrypoint module spec

## Event Routing Table (Normative)

| GitHub Event | ActionMode | Engine Call |
|-------------|-----------|-------------|
| `workflow_dispatch` with `mode: run` | Run | `OrchestratorService::run()` |
| `workflow_dispatch` with `mode: validate` | Validate | `ValidationLoopService::validate()` |
| `workflow_dispatch` with `mode: plan` | Plan | `OrchestratorService::plan_only()` |
| `issue_comment` containing `/rigorix run` | Run | `OrchestratorService::run()` |
| `issue_comment` containing `/rigorix validate` | Validate | `ValidationLoopService::validate()` |
| `pull_request` opened/synchronize | Validate | `ValidationLoopService::validate()` |
| `push` (any branch) | Status | `OrchestratorService::status()` |

---

*Date: 2026-06-20*
*Session: rigorix-oss design session*
