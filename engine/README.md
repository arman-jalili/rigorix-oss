# rigorix-engine

**Template-driven DAG execution engine — all business logic lives here.**

The engine is the core library containing **30 modules** — 27 full Clean Architecture bounded contexts plus 3 cross-cutting modules. It has zero CLI or GitHub-specific code; both the CLI and GitHub Action consume it as a library.

---

## Architecture

Every module follows the same Clean Architecture layering:

```
module/
├── domain/           # Entities, value objects, error enums, trait interfaces
├── application/      # Service traits, DTOs, factory interfaces
│   ├── service.rs    # Use-case traits (e.g., ParallelExecutionService)
│   ├── dto/          # Input/output data transfer objects
│   └── factory.rs    # Abstract factory traits for DI
├── infrastructure/   # Repository trait interfaces (implementations in sibling files)
├── interfaces/       # API contracts (HTTP, events)
└── mod.rs            # Module root
```

### Bounded Contexts

#### Phase 0 — Foundation

| Module | Purpose | Key Types |
|--------|---------|-----------|
| `configuration` | Multi-source config loading (TOML + env + defaults), Secret wrapper | `Config`, `Secret`, `LlmProvider` |
| `cancellation` | Graceful/immediate shutdown via tokio watch channels | `CancellationToken`, `ShutdownSignal` |
| `audit` | HMAC-signed audit envelopes with circuit breaker | `AuditEnvelope`, `CircuitBreakerState` |
| `execution_engine` | Parallel DAG execution with retry logic | `ParallelExecutor`, `ExecutionResult`, `RetryPolicy` |
| `failure_classification` | Failure type categorization + retry strategy selection | `FailureType`, `RetryStrategy`, `BackoffStrategy` |
| `failure_parser` | Parse failure output with tree-sitter, extract diagnostics, suggest fixes | `ParsedFailure`, `FixSuggestion`, `SourceLocation` |
| `error` | Root error type aggregating all 18 sub-errors | `CoreOrchestratorError` |

#### Phase 1 — Planning

| Module | Purpose | Key Types |
|--------|---------|-----------|
| `planning` | 6-phase LLM planning: classify → extract → generate → validate → hash | `PlanningPipeline`, `PlanningResult`, `UserIntent` |
| `templates` | TOML template parsing and rendering | `Template`, `TemplateEngine`, `TemplateParser` |
| `template_generation` | LLM-based template generation from natural language | `TemplateGenerator`, `GenerationService` |
| `dag_engine` | DAG construction (add_unchecked → seal), topo sort, cycle detection | `TaskGraph`, `TaskNode`, `PlanDiff`, `ImpactLevel` |
| `plan_validation` | Self-correcting validate loop: plan → execute → verify → repeat | `ValidationLoopService`, `ValidationState` |
| `llm_step` | LLM generation nodes in the DAG | `LlmGenerateNode`, `LlmStepContext` |
| `orchestrator` | Top-level entry point wiring planning → execution → persistence → audit | `OrchestratorService`, `ExecutionRecord`, `OrchestratorConfig` |

#### Phase 2 — Execution & Tools

| Module | Purpose | Key Types |
|--------|---------|-----------|
| `tools` | Tool trait + 9 implementations: file, git, command, LSP | `Tool` trait, `ToolRegistry`, `ToolInput` |
| `enforcement` | Execution limits (concurrency, tool calls, budget) | `ExecutionEnforcer`, `EnforcementConfig` |
| `risk_gating` | Tool execution risk classification (Low/Med/High) | `RiskClassifier`, `RiskConfig`, `RiskLevel` |
| `budget_tracking` | RAII-style LLM call/token budget reservations | `LlmBudgetService`, `LlmBudgetReservation` |

#### Phase 3 — Governance

| Module | Purpose | Key Types |
|--------|---------|-----------|
| `policy_engine` | Lane-based policy evaluation (DiffScope, Blocker, ReviewStatus) | `PolicyEngineService`, `LaneContext` |
| `quality_gates` | Post-execution quality evaluation (GreenContract) | `QualityGateService`, `GreenContract`, `QualityLevel` |
| `permission` | Path-based permission enforcer for tool access | `PermissionEnforcer`, `BashClassifier`, `Policy` |
| `hooks` | Pre/post execution hooks with filesystem repository | `HookRunner`, `HookConfig`, `HookResult` |
| `recovery_recipes` | Scenario-based recovery with escalation | `RecoveryRecipe`, `FailureScenario`, `EscalationPath` |
| `code_gen` | Code generation result domain for DAG node outputs | `CodeGenResult`, `SyntaxGate`, `ValidationOutcome` |

#### Phase 4 — Infrastructure

| Module | Purpose | Key Types |
|--------|---------|-----------|
| `event_system` | 11-variant ExecutionEvent, broadcast + persistence | `ExecutionEvent`, `EventBus`, `PersistedEvent` |
| `state_persistence` | Filesystem-backed execution state save/load | `ExecutionState`, `StateManager`, `ExecutionRecord` |
| `observability` | Prometheus metrics, health checks, tracing | `MetricsRegistry`, `HealthService`, `TracingConfig` |
| `code_graph` | Symbol graph builder using tree-sitter (Rust/TS/Python) | `CodeGraph`, `SymbolNode`, `GraphMetadata` |
| `repo_engine` | Workspace-level symbol graph + validation | `SymbolGraph`, `SymbolWorkspace` |

#### Cross-Cutting

| Module | Purpose | Key Types |
|--------|---------|-----------|
| `common` | Shared validation helpers and utility functions | `ValidationResult`, `ValidationError` |

---

## Core Data Flow

```
┌─────────────────────────────────────────────────────────────┐
│ OrchestratorService (orchestrator crate)                     │
│                                                              │
│  run(RunInput) {                                             │
│    1. Generate execution_id (UUIDv7)                         │
│    2. Publish PlanningStarted event                          │
│    3. planning_pipeline.plan_with_graph(intent)               │
│    4. Publish PlanningCompleted event                        │
│    5. state_manager.save_state(Pending)                      │
│    6. execution_service.execute_graph(task_graph)            │
│    7. state_manager.save_state(Completed/Failed)             │
│    8. event_bus.drain_persisted() → ExecutionRecord          │
│    9. audit_service.build_and_send() (best-effort)           │
│  }                                                           │
└─────────────────────────────────────────────────────────────┘
```

---

## Error Handling

All 18 module-level error types are aggregated into `CoreOrchestratorError` via `#[from]`:

```rust
pub enum CoreOrchestratorError {
    Dag(DagError),
    Planning(PlanningError),
    Execution(ExecutionError),
    Configuration(ConfigurationError),
    // ... 14 more
}
```

Every error has:
- **`error_code()`** — machine-readable string (e.g., `"DAG_ERROR"`, `"PLANNING_ERROR"`)
- **`is_retriable()`** — whether the error may succeed on retry
- **`http_status()`** — recommended HTTP status code

---

## Testing

```bash
# Unit + integration tests
cargo test -p rigorix-engine

# With live LLM calls (requires API key)
cargo test -p rigorix-engine --features live-tests

# Benchmarks
cargo bench -p rigorix-engine
```

---

## Key Design Decisions

| ADR | Decision | Link |
|-----|----------|------|
| ADR-001 | Clean Architecture with bounded contexts | [Read](.pi/architecture/decisions/ADR-001-architecture-pattern.md) |
| ADR-002 | TOML template format for DAG definitions | [Read](.pi/architecture/decisions/ADR-002-toml-template-format.md) |
| ADR-003 | Async trait-based LLM provider abstraction | [Read](.pi/architecture/decisions/ADR-003-llm-provider-traits.md) |
| ADR-004 | Autonomy presets (Default, Advanced, Aggressive) | [Read](.pi/architecture/decisions/ADR-004-autonomy-presets.md) |
| ADR-005 | Event bus with broadcast + drain persistence | [Read](.pi/architecture/decisions/ADR-005-event-bus-persistence.md) |
| ADR-006 | Atomic write-rename for state persistence | [Read](.pi/architecture/decisions/ADR-006-atomic-write-rename.md) |
| ADR-007 | Risk gating with Low/Medium/High classification | [Read](.pi/architecture/decisions/ADR-007-risk-gating-model.md) |
| ADR-008 | RAII-style budget reservation for LLM calls | [Read](.pi/architecture/decisions/ADR-008-raii-budget-reservation.md) |

---

## License

MIT OR Apache-2.0
