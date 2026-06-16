# System Context — CLI wrapping Rigorix Engine

```mermaid
graph TB
    subgraph "cli crate (thin binary)"
        CLI[CLI Boundary<br/>command dispatch, TUI,<br/>config, signals, output]
    end

    subgraph "rigorix-engine crate (library)"
        CONFIG[Configuration]
        OBSERV[Observability]
        CANCEL[Cancellation]
        PLANNING[Planning Pipeline]
        TEMPLATES[Templates]
        TEMPGEN[Template Generation]
        REPO[Repo Engine]
        DAG[DAG Engine]
        EXEC[Execution Engine]
        TOOLS[Tool System]
        FAILURE[Failure Classification]
        ENFORCE[Enforcement]
        RISK[Risk Gating]
        BUDGET[Budget Tracking]
        EVENTS[Event System]
        AUDIT[Audit]
        STATE[State Persistence]
    end

    CLI --> CONFIG
    CLI --> OBSERV
    CLI --> CANCEL
    CLI --> PLANNING
    CLI --> TEMPGEN
    CLI --> TEMPLATES
    CLI --> EVENTS
    CLI --> AUDIT
    CLI --> STATE
    CLI --> EXEC

    PLANNING --> TEMPLATES
    PLANNING --> TEMPGEN
    PLANNING --> REPO
    PLANNING --> BUDGET
    PLANNING --> DAG
    DAG --> EXEC
    EXEC --> TOOLS
    EXEC --> CANCEL
    EXEC --> FAILURE
    EXEC --> ENFORCE
    ENFORCE --> RISK
    ENFORCE --> BUDGET
    EXEC --> EVENTS
    EVENTS --> AUDIT
    EVENTS --> STATE
```

## Key Principle

**The CLI crate has one module: `cli_boundary`.** All domain logic, execution, planning, templates, etc. live in the `rigorix-engine` crate. The CLI calls engine APIs directly — no wrapper traits, no mirror DTOs, no parallel domain layers.

## Dependency Flow

```
User → rigorix binary → clap parsing → dispatch → engine API → format output
```

*Updated: 2026-06-16*
*Reflects single-module CLI architecture*
