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
│  │       Approval Binding (intent verify pre-dispatch)     │    │
│  │  intent hash → verify → consume · IntentMismatch halt   │    │
│  │  effect-scope oracle (git diff) · signed records        │    │
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
│  │              Quality Gates & Scoring                      │    │
│  │  ┌─────────────────┐    ┌────────────────────────┐      │    │
│  │  │  Quality Gates   │    │  Scored Evaluation    │      │    │
│  │  │  (GreenContract) │    │  (pluggable backends) │      │    │
│  │  │  test scope      │    │  output quality       │      │    │
│  │  └─────────────────┘    └────────────────────────┘      │    │
│  └─────────────────────────────────────────────────────┘    │
│         │                                                        │
│         ▼                                                        │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │              Policy Engine                               │    │
│  │  ScoreAbove / ScoreBelow / GreenAt / ...                 │    │
│  │  → block_merge / flag_for_review / closeout              │    │
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
│  │  drain)      │   │ (atomic w/r) │   │  + scoring refs  │    │
│  └──────────────┘   └──────────────┘   └──────────────────┘    │
└─────────────────────────────────────────────────────────────────┘
```

## Module Layers

| Layer | Modules | Purpose | Entry Point |
|-------|---------|---------|-------------|
| Planning | planning-pipeline, template-system, template-generation, repo-engine, budget-tracking | Intent → validated plan | `rigorix/src/planning/` |
| Execution | dag-engine, execution-engine, risk-gating, tool-system, enforcement, cancellation, failure-classification, **quality-gates**, **scored-evaluation**, **approval** | Plan → execution → result + quality scoring + consequence-bound sign-off | `rigorix/src/dag/`, `rigorix/src/tools/`, `rigorix/src/quality/`, `rigorix/src/approval/` |
| Policy | policy-engine | Rule evaluation, merge gating, closeout | `rigorix/src/policy/` |
| Observability | event-system, state-persistence, audit | Events → state → audit trail (with scoring refs) | `rigorix/src/event_bus.rs`, `rigorix/src/state/` |
| Cross-Cutting | configuration, error-handling, **identity** | Config loading, error types, attributed human identity | `rigorix/src/config.rs`, `rigorix/src/error.rs`, `rigorix/src/identity/` |

## Module Dependency Graph

```
planning-pipeline
    ├── template-system         (template loading + generation)
    ├── template-generation     (LLM fallback on low confidence)
    ├── repo-engine             (symbol context for planning)
    └── budget-tracking         (LLM cost control)

dag-engine
    └── template-system         (consumes TaskGraph)

execution-engine
    ├── dag-engine              (consumes TaskGraph)
    ├── risk-gating             (tool gate checks)
    ├── tool-system             (tool execution)
    ├── enforcement             (hard cap checks)
    ├── cancellation            (shutdown signals)
    ├── failure-classification  (retry routing)
    │
    ├── quality-gates           (GreenContract evaluation)
    │   └── policy-engine       (GreenAt condition)
    │
    └── scored-evaluation       (output quality scoring)
        ├── policy-engine       (ScoreAbove/ScoreBelow conditions)
        ├── audit               (scoring_results envelope extension)
        └── event-system        (ScoredEvaluation* event variants)

approval
    ├── execution-engine        (pre-dispatch intent verification)
    ├── identity                (approver identity, token_claims_ref)
    ├── audit                   (approval_events, scope_violations, decision_context_ref)
    ├── state-persistence       (durable approval records)
    └── failure-classification  (IntentMismatch — non-retriable)

identity
    ├── orchestrator            (RunInput.identity — author)
    ├── approval                (approver_id, token_claims_ref)
    └── audit                   (envelope identity block)

policy-engine
    └── quality-gates           (evaluates GreenAt)
    └── scored-evaluation       (evaluates ScoreAbove/ScoreBelow)

event-system
    ├── execution-engine        (publishes node events)
    ├── planning-pipeline       (publishes plan events)
    ├── enforcement             (publishes budget warnings)
    ├── quality-gates           (publishes gate events)
    ├── scored-evaluation       (publishes scoring events)
    │
    ├── state-persistence       (drains events into records)
    └── audit                   (builds envelopes from events)

configuration ──► all modules (config shared via Arc)
error-handling ──► all modules (thiserror enums)
```

## Data Flow Overview

### Request Flow (CLI Invocation with Quality Scoring)

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
  │
  ├── Quality Gates: evaluate GreenContract(test_scope)
  │     └── If scored_evaluation node exists → invoke scoring
  │           ├── Resolve backend (MCP/HTTP/Local)
  │           ├── Emit ScoredEvaluationStarted
  │           ├── Evaluate artifact against rubric
  │           ├── Emit ScoredEvaluationCompleted/Failed
  │           └── Persist scoring result
  │
  ├── Approval Binding: for approved nodes, verify intent pre-dispatch
  │     ├── Hash match → dispatch + consume (single-use, TTL)
  │     ├── Hash mismatch → HALT (IntentMismatch) — re-approval required
  │     └── Post-execution: effect-scope vs git-diff oracle → scope_violations
  │
  ├── Policy Engine: evaluate ScoreAbove/ScoreBelow/GreenAt
  │     └── If gating rule matches → block_merge / flag_for_review
  │
  └── Vec<TaskResult>
  │
  ▼
StateManager::save_state(final)
EventBus::drain_persisted() → ExecutionRecord
  ├── Includes scoring results in audit envelope
  └── Includes quality gate outcomes
```

### Event Flow (with Scoring Events)

```
Every component publishes to EventBus:
  PlanningStarted  →  PlanningCompleted
  NodeStarted      →  NodeCompleted/Failed/Retrying
  ToolExecuted
  BudgetWarning

  ScoredEvaluationStarted  →  ScoredEvaluationCompleted/Failed
  QualityGateEvaluated     →  QualityGateOutcome

  ApprovalRecorded        →  IntentMismatchDetected  →  ScopeViolationRecorded

  PolicyRuleMatched        →  ActionsDispatched

  ExecutionCompleted / Failed / Cancelled

Subscribers:
  ConsoleEventPrinter → human-readable stdout
  TUI subscriber      → ratatui real-time views
  State Persistence   → drained into ExecutionRecord at end
  Audit               → built into AuditEnvelope (with scoring refs)
```

## Security Boundaries

| Boundary | Enforcement | Module |
|----------|-------------|--------|
| User → CLI | No auth (local CLI) | cli |
| Tool → Filesystem | Path validation against repo_root | tool-system |
| Tool → Shell | RunCommand allowlist + High risk dry-run | risk-gating, tool-system |
| LLM Provider → Planning | API key via Secret wrapper | configuration |
| Human Approval → Node | Intent-hash binding + pre-dispatch verification | approval |
| Identity → Evidence | Attributed claims; best-effort verification | identity |
| Events → Audit | HMAC envelope signing | audit |
| Scoring Payload → Backend | HMAC-signed payloads for MCP/HTTP | scored-evaluation |
| Scoring Script → Host | Script path allowlist validation | scored-evaluation |

---

*Last updated: 2026-08-28*
*Architecture version: 1.2.0*
