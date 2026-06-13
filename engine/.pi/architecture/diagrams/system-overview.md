# System Architecture Overview

<!--
Canonical Reference: .pi/architecture/diagrams/system-overview.md
Blueprint Source: Domain Exploration Session 63c25384
-->

## High-Level Architecture

Rigorix is a **deterministic coding CLI** — a task graph compiler with execution profiles. It is NOT a web service, API gateway, or multi-agent system.

```
┌─────────────────────────────────────────────────────────────────┐
│                         User (Developer)                         │
│                   (CLI / TUI / GitHub Action)                    │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                        Planning Phase                            │
│                                                                  │
│  ┌──────────────┐   ┌──────────────┐   ┌──────────────────┐    │
│  │    Config    │   │  Repo Engine │   │   Budget Check   │    │
│  │  (loading)   │   │ (index code) │   │   (RAII reserve) │    │
│  └──────────────┘   └──────────────┘   └──────────────────┘    │
│         │                   │                     │              │
│         ▼                   ▼                     ▼              │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │              Planning Pipeline                           │    │
│  │  Classify → (Generate if low confidence) → Extract      │    │
│  │  → Generate TaskGraph → Validate → PlanOutput           │    │
│  └─────────────────────────────────────────────────────────┘    │
│         │                                                        │
│         ▼                                                        │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │              Template System + Generator                  │    │
│  │  (TOML parsing, built-in templates, LLM generation)      │    │
│  └─────────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                        Execution Phase                           │
│                                                                  │
│  ┌──────────────┐   ┌──────────────┐   ┌──────────────────┐    │
│  │   DAG Engine │   │   Risk Gate  │   │   Enforcement    │    │
│  │  (topo sort) │   │ (Low/Med/High)│   │ (hard caps)     │    │
│  └──────────────┘   └──────────────┘   └──────────────────┘    │
│         │                   │                     │              │
│         ▼                   ▼                     ▼              │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │              ParallelExecutor                            │    │
│  │  (tokio JoinSet, configurable concurrency)               │    │
│  └─────────────────────────────────────────────────────────┘    │
│         │                                                        │
│         ▼                                                        │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │              Tool System                                 │    │
│  │  FileRead · FileWrite · FileAppend · FilePatch           │    │
│  │  RunCommand · LspQuery · GitRead · GitStage · GitCommit  │    │
│  └─────────────────────────────────────────────────────────┘    │
│         │                                                        │
│         ▼                                                        │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │              Cancellation Manager                        │    │
│  │  (Graceful / Immediate shutdown signals)                 │    │
│  └─────────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                     Observability & Persistence                   │
│                                                                  │
│  ┌──────────────┐   ┌──────────────┐   ┌──────────────────┐    │
│  │   Event Bus  │──►│    State     │──►│      Audit       │    │
│  │ (broadcast + │   │ Persistence  │   │   (envelopes)    │    │
│  │  drain)      │   │ (atomic w/r) │   │                  │    │
│  └──────────────┘   └──────────────┘   └──────────────────┘    │
└─────────────────────────────────────────────────────────────────┘
```

## Module Layers

| Layer | Modules | Purpose | Entry Point |
|-------|---------|---------|-------------|
| Planning | planning-pipeline, template-system, template-generation, repo-engine, budget-tracking | Intent → validated plan | `rigorix/src/planning/` |
| Execution | dag-engine, execution-engine, risk-gating, tool-system, enforcement, cancellation, failure-classification | Plan → execution → result | `rigorix/src/dag/`, `rigorix/src/tools/` |
| Observability | event-system, state-persistence, audit | Events → state → audit trail | `rigorix/src/event_bus.rs`, `rigorix/src/state/` |
| Cross-Cutting | configuration, error-handling | Config loading, error types | `rigorix/src/config.rs`, `rigorix/src/error.rs` |

## Module Dependency Graph

```
planning-pipeline
    ├── template-system     (template loading + generation)
    ├── template-generation (LLM fallback on low confidence)
    ├── repo-engine         (symbol context for planning)
    └── budget-tracking     (LLM cost control)

dag-engine
    └── template-system     (consumes TaskGraph)

execution-engine
    ├── dag-engine          (consumes TaskGraph)
    ├── risk-gating         (tool gate checks)
    ├── tool-system         (tool execution)
    ├── enforcement         (hard cap checks)
    ├── cancellation        (shutdown signals)
    └── failure-classification (retry routing)

event-system
    ├── execution-engine    (publishes node events)
    ├── planning-pipeline   (publishes plan events)
    ├── enforcement         (publishes budget warnings)
    │
    ├── state-persistence   (drains events into records)
    └── audit               (builds envelopes from events)

configuration ──► all modules (config shared via Arc)
error-handling ──► all modules (thiserror enums)
```

## Data Flow Overview

### Request Flow (CLI Invocation)

```
UserIntent ("add a migration script runner")
  │
  ├── Config.load()
  ├── RepoEngine::index()
  ├── LlmBudget::new(config)
  │
  ▼
PlanningPipeline::plan_with_graph(intent, budget, symbols)
  ├── Budget pre-check
  ├── Classifier::classify_with_alternatives()
  │     └── Low confidence? → TemplateGenerator::generate()
  ├── ParameterExtractor::extract()
  ├── TemplateEngine::generate() → TaskGraph
  ├── CompositeValidator::validate()
  └── PlanningResult + TaskGraph
  │
  ▼
ParallelExecutor::execute(&mut graph, cancel_token)
  ├── Ready queue (topological order)
  ├── For each node: risk gate → tool execute → check result
  │     ├── Success → mark_completed, next ready node
  │     └── Failure → classify → can_retry? → retry/fallback/abort
  └── Vec<TaskResult>
  │
  ▼
StateManager::save_state(final)
EventBus::drain_persisted() → ExecutionRecord
```

### Event Flow

```
Every component publishes to EventBus:
  PlanningStarted  →  PlanningCompleted  →  NodeStarted  →  NodeCompleted  →  ...
                                                                    │
                                                                    ▼
                                                              ExecutionCompleted
                                                                  (or Failed/Cancelled)

Subscribers:
  ConsoleEventPrinter → human-readable stdout
  TUI subscriber      → ratatui real-time views
  State Persistence   → drained into ExecutionRecord at end
  Audit               → built into AuditEnvelope
```

## Security Boundaries

| Boundary | Enforcement | Module |
|----------|-------------|--------|
| User → CLI | No auth (local CLI) | cli |
| Tool → Filesystem | Path validation against repo_root | tool-system |
| Tool → Shell | RunCommand allowlist + High risk dry-run | risk-gating, tool-system |
| LLM Provider → Planning | API key via Secret wrapper | configuration |
| Events → Audit | HMAC envelope signing | audit |

---

*Last updated: 2026-06-13*
*Architecture version: 1.0.0*
