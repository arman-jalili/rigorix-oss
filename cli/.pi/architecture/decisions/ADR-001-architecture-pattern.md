# ADR-001: Domain-Driven Design with Bounded Contexts

**Status:** Accepted
**Date:** 2026-06-16
**Session:** 71e2b81a-a7a1-48ee-ab8f-56284bbec92d

## Context

The domain exploration identified 18 bounded contexts that must be implemented as independently evolvable modules. The CLI module wraps the existing `rigorix-engine` crate (which already follows Clean Architecture with bounded contexts).

Key architectural questions:
1. How do the 18 contexts relate to each other and to the existing engine crate?
2. What dependency direction rules apply?
3. How do contexts communicate?

## Decision

We will use **Domain-Driven Design with Bounded Contexts** as independently evolvable modules following a **Modular Monolith** pattern with **Clean Architecture** layering.

### Layer Architecture

```
┌──────────────────────────────────────────────┐
│              CLI Boundary                     │  ─── CLI crate
│  (commands, TUI, output formatting)           │      (depends on engine)
├──────────────────────────────────────────────┤
│            ┌──────────────────────┐           │
│            │   Application        │           │
│            │   (services, DTOs)   │           │  ─── rigorix-engine crate
│            ├──────────────────────┤           │      (library)
│            │   Domain             │           │
│            │   (entities, events) │           │
│            ├──────────────────────┤           │
│            │   Infrastructure     │           │
│            │   (repositories)     │           │
│            └──────────────────────┘           │
└──────────────────────────────────────────────┘
```

### Key Rules

1. **CLI crate depends on engine crate** — never the reverse. The CLI is a thin wrapper.
2. **Engine domains follow Clean Architecture** — domain → application → infrastructure → interfaces (inward dependency rule)
3. **Cross-context communication** — via domain events through EventBus, not direct calls
4. **Contract freeze** — engine interfaces are frozen; implementation must satisfy the contract
5. **Each bounded context owns its data** — no cross-context database access

### Context Dependencies

See `diagrams/system-context.md` for the full dependency graph. Key rules:
- Foundation contexts (Configuration, Observability, Cancellation) have no dependencies on higher layers
- Safety contexts (Enforcement, Risk Gating, Budget Tracking) depend only on Foundation
- Planning contexts depend on Foundation + Repo Engine
- Execution contexts depend on Foundation + Safety + Planning (DAG)
- Event System is the backbone — all contexts emit to it, TUI subscribes

## Consequences

### Positive
- **Clear boundaries**: Each context is independently testable and evolvable
- **Engine reuse**: Engine contracts are frozen and tested — CLI only needs integration
- **TUI decoupling**: TUI subscribes to EventBus — never blocks execution
- **Migration path**: Existing engine code maps 1:1 to module docs

### Negative
- **Boilerplate**: Each context needs its own module doc even when only wrapping engine
- **Event overhead**: Cross-context communication via events adds indirection
- **Contract discipline**: Changes to engine contracts require coordinated updates

### Neutral
- **Modular monolith**: Can extract contexts to separate services if needed later
- **DDD rigor**: Requires disciplined adherence to ubiquitous language

## Alternatives Considered

| Alternative | Rejected Because |
|-------------|-----------------|
| Monolith without domain boundaries | No separation of concerns, violates single responsibility |
| Microservices from day one | Over-engineering for initial scope. Extract later if needed. |
| Layered Architecture (3-tier) | Does not enforce domain boundaries across bounded contexts |
| Flat crate with no module structure | Impossible to maintain with 18 contexts |

## Affected Modules

All 18 bounded contexts (see system-context.md for dependency graph):
1. **CLI Boundary** — CLI crate, depends on engine
2. **Configuration** — Engine context, foundation layer
3. **Planning Pipeline** — Engine context, planning layer
4. **Templates** — Engine context, planning layer
5. **Template Generation** — Engine context, planning layer
6. **DAG Engine** — Engine context, execution layer
7. **Execution Engine** — Engine context, execution layer
8. **Tool System** — Engine context, execution layer
9. **Enforcement** — Engine context, safety layer
10. **Risk Gating** — Engine context, safety layer
11. **Budget Tracking** — Engine context, safety layer
12. **Cancellation** — Engine context, foundation layer
13. **Failure Classification** — Engine context, execution layer
14. **Event System** — Engine context, observability layer
15. **Audit** — Engine context, observability layer
16. **State Persistence** — Engine context, observability layer
17. **Observability** — Engine context, foundation layer
18. **Repo Engine** — Engine context, planning layer

## Implementation Notes

- The CLI crate (`cli/`) should be a thin binary crate with `rigorix-engine` as a dependency
- No Cross-context database access: the engine's repository interfaces remain the sole persistence path
- The EventBus subscription model means TUI rendering never blocks or slows execution
