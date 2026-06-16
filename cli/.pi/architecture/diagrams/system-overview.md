# System Architecture Overview

## Architecture Pattern

**Thin binary CLI wrapping a modular monolith library.** The CLI is a single-module binary (`rigorix-cli`) that depends on the `rigorix-engine` library crate. The engine contains 17 bounded contexts, each following Clean Architecture (domain → application → infrastructure → interfaces).

```
┌──────────────────────────────────────────────────────┐
│              rigorix-cli (binary crate)               │
│                                                      │
│  ┌────────┐  ┌──────────┐  ┌────────┐  ┌──────────┐ │
│  │ clap   │  │ config   │  │ signal │  │ tracing  │ │
│  │ parse  │  │ merge    │  │ handler│  │ init     │ │
│  └───┬────┘  └────┬─────┘  └───┬────┘  └────┬─────┘ │
│      │            │            │            │       │
│      └────────────┼────────────┼────────────┘       │
│                   ▼            ▼                    │
│        ┌────────────────────────────┐              │
│        │     dispatch (main.rs)     │              │
│        │  command → engine → format │              │
│        └───────────┬────────────────┘              │
│                    │                               │
└────────────────────┼───────────────────────────────┘
                     │
                     ▼
┌──────────────────────────────────────────────────────┐
│            rigorix-engine (library crate)              │
│                                                       │
│  ┌─────────┐ ┌──────────┐ ┌───────────┐ ┌─────────┐ │
│  │Planning │ │Execution │ │ Templates │ │ Audit   │ │
│  │Pipeline │ │  Engine  │ │ & Gen     │ │ & State │ │
│  └─────────┘ └──────────┘ └───────────┘ └─────────┘ │
│  ┌─────────┐ ┌──────────┐ ┌───────────┐ ┌─────────┐ │
│  │  Event  │ │Budgets   │ │Enforce    │ │Risk     │ │
│  │  System │ │& Cancel  │ │ment       │ │Gating   │ │
│  └─────────┘ └──────────┘ └───────────┘ └─────────┘ │
│                                                       │
│  All 17 bounded contexts, each with Clean Architecture│
│  (domain → application → infrastructure → interfaces) │
└──────────────────────────────────────────────────────┘
```

## Key Rule

Per ADR-002, the CLI depends on the engine — never the reverse. The CLI is a thin wrapper.

## What the CLI Handles

| Concern | Implementation |
|---------|---------------|
| Command parsing | Clap argument parser |
| Config loading | TOML + env + flags → engine Config |
| Signal handling | Ctrl+C → engine CancellationToken |
| Tracing | tracing-subscriber init |
| Output formatting | Pretty, JSON, Quiet |
| TUI rendering | Ratatui (Phase 2) |

## What the Engine Handles

All business logic: planning, execution, templates, template generation, DAG construction, enforcement, risk gating, budget tracking, failure classification, event system, audit trails, state persistence, observability, cancellation signals, repo indexing, tool registration.
