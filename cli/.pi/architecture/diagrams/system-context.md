# System Context — CLI wrapping Rigorix Engine

```mermaid
graph TB
    subgraph "cli crate (thin binary)"
        CLI_BOUNDARY[cli_boundary<br/>dispatch, config,<br/>signals, output]
        TUI[tui<br/>dashboard, views,<br/>widgets, event bridge]
    end

    subgraph "rigorix-engine crate (library)"
        ORCH[Orchestrator]
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

    CLI_BOUNDARY --> ORCH
    CLI_BOUNDARY --> CONFIG
    CLI_BOUNDARY --> OBSERV
    CLI_BOUNDARY --> CANCEL
    CLI_BOUNDARY --> STATE
    CLI_BOUNDARY --> DAG
    CLI_BOUNDARY --> AUDIT
    CLI_BOUNDARY --> TEMPLATES
    CLI_BOUNDARY --> TEMPGEN

    TUI --> EVENTS
    TUI --> STATE

    ORCH --> PLANNING
    ORCH --> EXEC
    ORCH --> STATE
    ORCH --> CANCEL
    ORCH --> EVENTS
    ORCH --> AUDIT
    ORCH --> BUDGET

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

## Entry Point Flow

```
$ rigorix  (no args)
    → tui::run(config)
    → user types intent in command bar
    → tokio::spawn(orchestrator.run(RunInput { intent }))
    → EventBridge subscribes to orchestrator.event_bus()
    → UI updates in real-time, user can cancel/inspect
    → orchestrator completes → return to command bar

$ rigorix run "add auth"  (with args)
    → cli_boundary::dispatch(Run, config)
    → build orchestrator, run, format output, print to stdout
    → exit
```

## Key Principle

**Two peer modules in the CLI crate:**
- `cli_boundary/` — dispatch, config, signals, output formatting
- `tui/` — terminal UI dashboard, EventBridge, views, widgets, input

All domain logic, execution, planning, templates, etc. live in the `rigorix-engine` crate. The CLI calls engine APIs directly — no wrapper traits, no mirror DTOs, no parallel domain layers.

## Dependency Flow

```
User → rigorix binary
    │
    ├── cli_boundary::dispatch() → engine API → format output
    │
    └── tui::run() → subscribe to EventBus → render views
```

## Module Boundaries

| Module | Depends On | Knows About Engine |
|--------|-----------|-------------------|
| `cli_boundary` | engine (orchestrator, config, state, audit, cancellation, dag, templates, template_generation) | Many engine types |
| `tui` | engine (event_system::EventBus, state_persistence) | Only EventBus + ExecutionState |

The TUI's engine surface is intentionally minimal — it reads two things:
- `EventBus` for real-time events
- `StateManager` for past execution loading

*Updated: 2026-06-16*
*Reflects two-module CLI architecture*
