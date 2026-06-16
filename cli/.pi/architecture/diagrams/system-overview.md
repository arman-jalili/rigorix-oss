# System Architecture Overview

<!--
Canonical Reference: .pi/architecture/diagrams/system-overview.md
-->

## Architecture Pattern

**Modular Monolith with Clean Architecture layers.** The system is one binary crate (`rigorix`) that depends on the `rigorix-engine` library crate. The engine contains 17 bounded contexts, each following Clean Architecture (domain → application → infrastructure → interfaces).

```
┌──────────────────────────────────────────────────────────────────┐
│                     rigorix (CLI binary crate)                    │
│                                                                  │
│  ┌─────────────┐  ┌──────────┐  ┌──────────┐  ┌──────────────┐ │
│  │  Commands   │  │   TUI    │  │  Output  │  │   Config     │ │
│  │  (clap)     │  │ (ratatui)│  │ (JSON)   │  │   Merge      │ │
│  └──────┬──────┘  └────┬─────┘  └────┬─────┘  └──────┬───────┘ │
│         │              │             │               │          │
└─────────┼──────────────┼─────────────┼───────────────┼──────────┘
          │              │             │               │
          ▼              ▼             ▼               ▼
┌──────────────────────────────────────────────────────────────────┐
│                    rigorix-engine (library crate)                  │
│                                                                  │
│  ┌────────────────────────────────────────────────────────────┐  │
│  │                    Orchestrator                             │  │
│  │  (wires all contexts together for a single execution run)   │  │
│  └────┬─────┬─────┬──────┬──────┬──────┬──────┬──────┬───────┘  │
│       │     │     │      │      │      │      │      │          │
│  ┌────▼┐ ┌──▼──┐ ┌▼────┐ ┌▼────┐ ┌▼────┐ ┌▼────┐ ┌▼────┐ ┌───▼─┐│
│  │Plan │ │DAG  │ │Exec │ │Tool │ │Enf  │ │Risk │ │Canc │ │Audit││
│  │Pipe │ │Eng  │ │Eng  │ │Sys  │ │orce │ │Gate │ │el   │ │     ││
│  └─────┘ └─────┘ └─────┘ └─────┘ └─────┘ └─────┘ └─────┘ └─────┘│
│  ┌────▼┐ ┌──────┐ ┌─────┐ ┌──────┐ ┌─────┐ ┌──────┐ ┌──────┐   │
│  │Templ│ │Templ │ │Fail │ │Event │ │State│ │Obser │ │Budget│   │
│  │ates │ │Gen   │ │Class│ │Sys   │ │Pers │ │vabil │ │Track │   │
│  └─────┘ └──────┘ └─────┘ └──────┘ └─────┘ └──────┘ └──────┘   │
│  ┌────────────────────────────────────────────────────────────┐  │
│  │                   Repo Engine                               │  │
│  │  (multi-language symbol indexing via tree-sitter)           │  │
│  └────────────────────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────────────────────┘
```

## Execution Flow (End-to-End)

```
User: rigorix run "refactor auth module"
  │
  ├─▶ CLI parses command, loads rigorix.toml
  ├─▶ CLI merges config (flags > env > file > defaults)
  ├─▶ CLI initializes tracing, signal handlers
  ├─▶ CLI creates ExecutionSession
  │
  ├─▶ [Planning Phase]
  │     ├─▶ Budget pre-check (≥2 LLM calls remaining?)
  │     ├─▶ Intent classification (LLM: match intent → template)
  │     ├─▶ Parameter extraction (LLM: fill template params)
  │     ├─▶ Graph generation (template + params → TaskGraph)
  │     ├─▶ Plan validation (CompositeValidator)
  │     └─▶ Hash computation (SHA-256 for audit)
  │
  ├─▶ [Execution Phase]
  │     ├─▶ Seal TaskGraph (topological sort, cycle detection)
  │     ├─▶ Parallel execution (tokio JoinSet, max N concurrent)
  │     │     ├─▶ Dequeue ready node → resolve tool → gate via Enforcer
  │     │     ├─▶ Execute tool → on failure: classify → retry/fallback/skip
  │     │     └─▶ Emit ExecutionEvent for each transition
  │     ├─▶ Track per-node state (status, retries, timing)
  │     └─▶ Aggregate ExecutionResult
  │
  ├─▶ [Post-Execution]
  │     ├─▶ Persist state (atomic write-rename)
  │     ├─▶ Emit terminal event (Completed/Failed/Cancelled)
  │     ├─▶ Send audit envelope (with retry + circuit breaker)
  │     └─▶ Render final output (TUI or JSON)
  │
  └─▶ CLI exits (ephemeral — no daemon)
```

## Layer Architecture (per bounded context)

All engine contexts follow the same Clean Architecture structure:

```
context/
├── domain/           # Pure domain entities, value objects, events
│   └── mod.rs
├── application/      # Service traits, DTOs, factory interfaces
│   ├── service.rs
│   ├── factory.rs
│   └── dto/
├── infrastructure/   # Repository interfaces (implementations elsewhere)
│   └── repository/
└── interfaces/       # API contracts (HTTP, events)
    └── http/
```

Rules:
- **Domain** depends on nothing — pure Rust structs + serde
- **Application** depends on domain
- **Infrastructure** depends on application
- **Interfaces** depends on application

## Key Architectural Decisions

| Decision | Choice | ADR |
|----------|--------|-----|
| Architecture pattern | Domain-Driven Design with Bounded Contexts | ADR-001 |
| CLI/engine split | Binary crate wraps library crate | ADR-002 |
| TUI framework | ratatui (async, tokio-compatible) | ADR-003 |
| Template format | TOML | ADR-004 |
| Cross-context comm | EventBus (tokio broadcast) | ADR-005 |
| Plugin system | Deferred to v2 (configurable RunCommand aliases for v1) | ADR-006 |
| Daemon mode | Deferred to v2 (ephemeral CLI for v1) | ADR-007 |
| State persistence | Atomic write-rename | ADR-008 |
| LLM provider | Claude via Anthropic Messages API | ADR-009 |
| Template generation persistence | Auto-persist fallback-generated templates | ADR-010 |

## Security Boundaries

| Boundary | Enforcement | Context |
|----------|-------------|---------|
| Tool execution | RiskGate: Low=auto, Medium=confirm, High=block/dry-run | Risk Gating |
| File system access | Path allowlists + deny-lists | Enforcement |
| API keys | Secret type (redacted Debug/Display), env var only | Configuration |
| Audit trail | Append-only planning hash chain | Audit |
| Budget | Hard caps on tokens, calls, execution time | Budget Tracking |

---

*Version: 1.0.0*
*Last updated: 2026-06-16*
