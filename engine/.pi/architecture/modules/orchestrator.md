# Orchestrator Architecture

<!--
Canonical Reference: .pi/architecture/modules/orchestrator.md
Blueprint Source: Redesigned CLI architecture (2026-06-16)
-->

## Overview

Top-level entry point that wires the full Rigorix execution lifecycle into a single operation. Sequences: config loading → planning (via PlanningPipeline) → TaskGraph execution (via ExecutionEngine) → state persistence (via StateManagerService) → event emission (via EventBus) → audit envelope building (via AuditService).

The Orchestrator exists so that any consumer (CLI, CI/CD, IDE plugin) can run a complete execution with one call. Without it, each consumer would need to wire 5+ engine services manually.

## Responsibilities

- Accept a `UserIntent` and `Config`, produce an `ExecutionRecord`
- Sequence the 6-phase execution lifecycle (see Data Flow)
- Manage execution_id allocation (UUIDv7)
- Coordinate cancellation signal propagation across all sub-services
- Build and return a complete `ExecutionRecord` with audit metadata
- Handle partial failures (e.g., audit send is best-effort)

## Data Flow — The `run()` Lifecycle

```
UserIntent + Config
    │
    ▼
┌─────────────────────────────────────────────────────────┐
│  1. Generate execution_id (UUIDv7)                      │
│  2. Publish PlanningStarted event                       │
│  3. Run PlanningPipeline::plan_with_graph(intent, budget)│
│  4. Publish PlanningCompleted event                     │
│  5. Save initial ExecutionState (Pending)               │
│  6. Execute DAG via ParallelExecutionService            │
│     (cooperative cancellation via CancellationToken)    │
│  7. Save final ExecutionState (Completed/Failed)        │
│  8. Drain EventBus → build ExecutionRecord              │
│  9. Send audit envelope (best-effort)                   │
│ 10. Return ExecutionRecord                              │
└─────────────────────────────────────────────────────────┘
```

### Cancellation Path

```
Ctrl+C (CLI) → SignalHandler
                    │
                    ▼
           CancellationService::request_graceful_shutdown()
                    │
                    ▼
           CancellationToken::cancel()
                    │
                    ▼
           ParallelExecutionService::abort_execution()
                    │
                    ▼
           StateManagerService::save_state(Cancelled)
```

## Components

| Component | File Path | Purpose | Canonical Section |
|-----------|-----------|---------|-------------------|
| OrchestratorService (trait) | `engine/src/orchestrator/application/service.rs` | Public interface: `run()`, `plan_only()`, `cancel()` | #orchestrator-service |
| OrchestratorServiceImpl | `engine/src/orchestrator/application/orchestrator_impl.rs` | Concrete implementation wiring 5+ engine services | #orchestrator-impl |
| OrchestratorBuilder | `engine/src/orchestrator/application/builder.rs` | Builder pattern for constructing Orchestrator from Config | #builder |
| OrchestratorConfig | `engine/src/orchestrator/domain/config.rs` | Orchestrator-specific config (event buffer size, audit toggle) | #config |
| ExecutionRecord | `engine/src/orchestrator/domain/record.rs` | Aggregate result: execution_id, planning metadata, task results, events | #record |
| OrchestratorError | `engine/src/orchestrator/domain/error.rs` | Error enum: planning, execution, state, audit, cancellation | #errors |
| OrchestratorEvent | `engine/src/orchestrator/domain/event/mod.rs` | High-level lifecycle events (RunStarted, RunCompleted, RunFailed) | #events |
| RunInput / RunOutput | `engine/src/orchestrator/application/dto/mod.rs` | DTOs for the top-level entry point | #dtos |

---

## Component Details

### OrchestratorService

**Purpose:** Single entry point for executing a Rigorix run from intent to result.

**Implementation File:** `engine/src/orchestrator/application/service.rs` (trait)

**Dependencies:**
- `PlanningPipelineService` — plan_with_graph()
- `ParallelExecutionService` — execute_graph()
- `StateManagerService` — save_state(), load_state()
- `CancellationService` — request_graceful_shutdown(), cancellation_token()
- `EventBus` — subscribe(), drain()
- `AuditService` — send_envelope()
- `LlmBudget` — budget checking

**Interface:**

```rust
#[async_trait]
pub trait OrchestratorService: Send + Sync {
    /// Full lifecycle: plan → execute → persist → emit → return record.
    async fn run(&self, input: RunInput) -> Result<RunOutput, OrchestratorError>;

    /// Plan only (no execution). Returns the plan for preview.
    async fn plan_only(&self, input: PlanOnlyInput) -> Result<PlanOnlyOutput, OrchestratorError>;

    /// Cancel a running execution.
    async fn cancel(&self, input: CancelInput) -> Result<CancelOutput, OrchestratorError>;

    /// Get current execution status.
    async fn status(&self) -> Result<StatusOutput, OrchestratorError>;

    /// Access the EventBus for subscriber registration (TUI, logs).
    fn event_bus(&self) -> &EventBus;
}
```

### OrchestratorBuilder

**Purpose:** Constructs an Orchestrator from a Config, wiring all internal dependencies.

**Implementation File:** `engine/src/orchestrator/application/builder.rs`

**Construction:**

```rust
let orchestrator = OrchestratorBuilder::new(config)
    .with_repo_root(repo_root)
    .with_enforcement_preset(enforcement_preset)
    .build()
    .await?;

let result = orchestrator.run(RunInput { intent }).await?;
```

### ExecutionRecord

**Purpose:** Complete output of a run, containing everything needed for audit, TUI, and persistence.

**Implementation File:** `engine/src/orchestrator/domain/record.rs`

| Field | Type | Description |
|-------|------|-------------|
| execution_id | Uuid | UUIDv7 execution identifier |
| planning | PlanningMetadata | Template, confidence, LLM calls/tokens, prompt hash |
| task_results | Vec\<TaskResult\> | Per-node results |
| events | Vec\<ExecutionEvent\> | Drained event log |
| context | ExecutionContext | Repo info, symbol graph hash |
| started_at | DateTime\<Utc\> | Execution start time |
| completed_at | Option\<DateTime\<Utc\>\> | Execution end time |
| duration_ms | u64 | Total wall-clock duration |

---

## DTOs

| DTO | Input/Output | Fields |
|-----|:-----------:|--------|
| RunInput | Input | intent: UserIntent, config: Config, repo_root: PathBuf |
| RunOutput | Output | execution_id, record: ExecutionRecord |
| PlanOnlyInput | Input | intent: UserIntent, config: Config, repo_root: PathBuf |
| PlanOnlyOutput | Output | plan: PlanningResult, graph: TaskGraph |
| CancelInput | Input | execution_id: Uuid, reason: Option\<String\> |
| CancelOutput | Output | execution_id, aborted: bool, nodes_cancelled: u32 |
| StatusOutput | Output | execution_id, status: ExecutionStatus, nodes: Vec\<NodeState\> |

---

## Error Handling

| Error Variant | Source | Recovery |
|---------------|--------|----------|
| PlanningFailed | PlanningPipeline | Retry with different intent |
| ExecutionFailed | ParallelExecutionService | Check task_results for partial completion |
| StatePersistenceFailed | StateManagerService | State may be recoverable from disk |
| CancellationFailed | CancellationService | Retry cancel |
| AuditFailed | AuditService | Non-fatal (best-effort) |
| OrchestratorInternal | Any sub-service | Bug — log and report |

---

## Dependencies

### Depends On
- **Planning Pipeline**: Intent → plan transformation
- **Execution Engine**: DAG execution with retry/cancellation
- **State Persistence**: Crash-safe state saves
- **Cancellation**: Graceful and immediate shutdown
- **Event System**: Event emission and draining
- **Audit**: Envelope building and delivery
- **Budget Tracking**: LLM budget pre-check
- **Configuration**: Config loading (used during setup, not per-run)

### Used By
- **CLI Boundary**: `rigorix run`, `rigorix plan` commands
- (Future) **CI/CD pipeline**, **IDE plugin**

---

## Testing Requirements

| Test Type | Coverage Target | Scenarios |
|-----------|----------------|-----------|
| Unit | 90%+ | Builder construction, error wrapping, DTO serialization |
| Integration | Full lifecycle | Mock services + real flow — 5+ scenarios |

**Key Test Scenarios:**
1. Happy path: plan → execute → persist → return record
2. Cancellation during execution: Ctrl+C → graceful shutdown → Cancelled state
3. Planning failure: low-confidence → fallback generation → success
4. State persistence failure: atomic write fails → error returned
5. Audit failure: non-fatal — record still returned

---

## Security Considerations

| Concern | Mitigation |
|---------|-----------|
| API key leakage in ExecutionRecord | Redact Secret fields before building record |
| Cancellation abuse | Only cancel execution matching execution_id |

---

## Performance Considerations

| Metric | Target | Notes |
|--------|--------|-------|
| Orchestrator overhead | < 5ms | Just event emission + state saving beyond sub-service time |
| Record build time | < 2ms | Event drain + envelope assembly |
| Memory per run | Proportional to event count | EventBus capacity bounds this |

---

## Change Log

| Date | Change | Section | Status |
|------|--------|---------|--------|
| 2026-06-16 | Initial architecture definition | all | done |
| 2026-06-16 | Contract freeze — all interfaces, DTOs, events, API contracts | interfaces, DTOs, events | done |
| 2026-06-16 | OrchestratorService implementation (#339) | orchestrator-impl | done |
| 2026-06-16 | OrchestratorBuilder implementation (#340) | builder | done |
| 2026-06-16 | ExecutionRecord helpers and tests (#341) | record | done |
| 2026-06-16 | Proofing scripts and CI integration (#342) | testing, CI | done |
| 2026-06-16 | Runbook, DR plan, and architecture sync (#343) | operations | done |
