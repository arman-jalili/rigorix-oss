# Execution Engine Architecture

<!--
Canonical Reference: .pi/architecture/modules/execution-engine.md
Blueprint Source: Domain Exploration Session 63c25384
Last Updated: 2026-11-01 — consumption refusal is classified from the ledger marker for the approval nonce and recorded on the refused run; a refusal writes one terminal event; approvals require binding and identity (#1230, #1232, #1235, #1263, #1267, #1271)
-->

## Overview

Executes sealed TaskGraphs from the DAG Engine using a concurrent worker pool (tokio JoinSet). Handles parallel node dispatch respecting dependency ordering, per-node retry loops with configurable strategies, cooperative cancellation, enforcement limits, and execution lifecycle events.

## Responsibilities

- Parallel node execution: dequeue ready nodes, dispatch to workers up to concurrency limit
- Inline retry loop per node: SameOperation → ExpandContext → AlternateApproach escalation
- Configurable backoff: Fixed, Exponential, Linear, Immediate
- Session management: execute, pause, resume, abort
- Cancellation integration: cooperative shutdown via CancellationToken
- Enforcement integration: concurrency limits, max failures before abort
- Fallback execution: execute fallback node when retries exhausted
- Execution event emission: 11 event types for observability
- Progress callbacks: per-terminal-state notifications for TUI
- Approval integration: pre-dispatch intent verification, `IntentMismatch` halt, consume-on-terminal (see approval module)

## Dependencies

- `dag_engine` — consumes sealed TaskGraphs
- `cancellation` — CancellationToken for graceful shutdown
- `enforcement` — ExecutionEnforcer for operation limits
- `risk_gating` — RiskClassifier for tool execution gates
- `tool_system` — Tool trait for executing node tool bindings
- `failure_classification` — FailureType classification
- `event_system` — EventBus for execution event emission
- `state_persistence` — ExecutionState for crash recovery
- `approval` — pre-dispatch intent verification, approval records (see `modules/approval.md`)

## Components

| Component | File Path | Purpose | Canonical Section |
|-----------|-----------|---------|-------------------|
| ParallelExecutorConfig | `engine/src/execution_engine/domain/parallel_executor.rs` | Global executor configuration (concurrency, retry defaults, enforcement) | #config |
| NodeExecutionState | `engine/src/execution_engine/domain/parallel_executor.rs` | Per-node lifecycle tracking (Pending → Ready → Running → Terminal) | #node-state |
| NodeStatus | `engine/src/execution_engine/domain/parallel_executor.rs` | Status enum: Pending, Ready, Running, Completed, Failed, Skipped, AwaitingApproval, IntentMismatch | #node-status |
| TaskResult | `engine/src/execution_engine/domain/parallel_executor.rs` | Single node execution result with output, duration, retry count | #task-result |
| ExecutionResult | `engine/src/execution_engine/domain/parallel_executor.rs` | Aggregate DAG execution result with summary statistics | #exec-result |
| RetryPolicy | `engine/src/execution_engine/domain/retry.rs` | Per-node retry config: max_attempts, strategies, backoff, skip conditions | #retry-policy |
| RetryStrategy | `engine/src/execution_engine/domain/retry.rs` | Strategy enum: SameOperation, ExpandContext, SimplifyOperation, AlternateApproach, SkipAndContinue | #retry-strategy |
| BackoffStrategy | `engine/src/execution_engine/domain/retry.rs` | Backoff enum: Fixed, Exponential, Linear, Immediate | #backoff |
| RetryDecision | `engine/src/execution_engine/domain/retry.rs` | Decision enum: Retry, Fallback, Skip, Abort | #retry-decision |
| FailureContext | `engine/src/execution_engine/domain/retry.rs` | Failure details for retry decision-making | #failure-context |
| ExecutionError | `engine/src/execution_engine/domain/error.rs` | Error enum: 10 variants with structured context | #errors |
| ExecutionEngineEvent | `engine/src/execution_engine/domain/event/mod.rs` | Event enum: 11 lifecycle event payloads | #events |
| ParallelExecutionService | `engine/src/execution_engine/application/service.rs` | Service trait: execute, pause, resume, abort, progress callbacks | #exec-service |
| RetryEvaluationService | `engine/src/execution_engine/application/service.rs` | Service trait: evaluate_retry, compute_backoff, validate_policy, decide | #retry-service |
| ParallelExecutionServiceImpl | `engine/src/execution_engine/application/service_impl.rs` | JoinSet-based concurrent executor with session management | #exec-impl |
| RetryEvaluationServiceImpl | `engine/src/execution_engine/application/service_impl.rs` | Stateless retry decision engine with strategy escalation | #retry-impl |
| ParallelExecutionFactory | `engine/src/execution_engine/application/factory.rs` | Factory trait for executor construction | #exec-factory |
| RetryEvaluationFactory | `engine/src/execution_engine/application/factory.rs` | Factory trait for retry service construction | #retry-factory |
| ExecutionResultRepository | `engine/src/execution_engine/infrastructure/repository/mod.rs` | Repository trait for execution result persistence | #exec-repo |
| RetryDecisionRepository | `engine/src/execution_engine/infrastructure/repository/mod.rs` | Repository trait for retry decision audit trail | #retry-repo |

---

## Component Details

### ParallelExecutorConfig

**Purpose:** Controls executor behaviour: concurrency, retry defaults, cancellation/enforcement integration

**Implementation File:** `engine/src/execution_engine/domain/parallel_executor.rs`

**Dependencies:**
- RetryPolicy (default_retry_policy fallback)

**Configuration Fields:**

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| max_concurrent_executions | u32 | 4 | Max nodes executing concurrently (0 = unlimited) |
| default_retry_policy | RetryPolicy | 4 attempts, exponential backoff | Fallback when node has no policy |
| enable_cancellation | bool | true | Check cancellation signals |
| enable_enforcement | bool | true | Apply enforcement limits |
| max_total_retries_per_session | u32 | 100 | Total retries across all nodes |
| max_failures_before_abort | u32 | 0 (unlimited) | Abort after N failures |
| enable_fallback | bool | true | Execute fallback nodes |
| enable_validation | bool | true | Run post-execution validation |

---

### ParallelExecutionService

**Purpose:** Parallel DAG execution with session management

**Implementation File:** `engine/src/execution_engine/application/service.rs` (trait)
`engine/src/execution_engine/application/service_impl.rs` (implementation)

**Methods:**

| Method | Input | Output | Description |
|--------|-------|--------|-------------|
| execute_graph | ExecuteGraphInput | ExecuteGraphOutput | Execute a sealed TaskGraph end-to-end |
| execute_node | ExecuteNodeInput | ExecuteNodeOutput | Execute single node with inline retry loop — runs the **same consequence-gate sequence** as `execute_graph` |
| get_execution_state | GetExecutionStateInput | GetExecutionStateOutput | Poll in-flight execution state |
| pause_execution | PauseExecutionInput | PauseExecutionOutput | Pause dispatch (in-flight nodes complete) |
| resume_execution | ResumeExecutionInput | ResumeExecutionOutput | Resume paused execution |
| abort_execution | AbortExecutionInput | AbortExecutionOutput | Cancel execution, skip remaining nodes |
| on_progress | callback | — | Register terminal-state notification callback |

---

### Inline Retry Loop
The retry loop is embedded directly in `execute_node()`, not a separate wrapper:

```
for attempt in 0..policy.max_attempts:
    1. Check skip conditions (if policy.has_skip_conditions)
    2. Check cancellation signal
    3. Execute node (ToolSystem)
    4. If success → return TaskResult
    5. Build FailureContext (failure_type, error, attempt, max_attempts)
    6. Evaluate retry via RetryEvaluationService.decide()
    7. Match decision:
       - Retry → compute backoff, sleep, loop
       - Fallback → execute fallback node, return
       - Skip → return skipped TaskResult
       - Abort → return aborted TaskResult
return exhausted TaskResult  (all attempts used)
```

---

### Retry Policy Defaults

```rust
RetryPolicy {
    max_attempts: 4,
    retry_strategies: [SameOperation, SameOperation, ExpandContext],
    backoff: Exponential(100ms base, 2.0x multiplier, 30s cap),
    enable_fallback: true,
    skip_on_exhaustion: false,
    retryable_failures: [],  // all failures retriable
}
```

---

### DTOs

| DTO | Input/Output | Fields |
|-----|:-----------:|--------|
| ExecuteGraphInput | Input | dag_id, config_override |
| ExecuteGraphOutput | Output | result, completed_at |
| ExecuteNodeInput | Input | dag_id, node_id, retry_policy |
| ExecuteNodeOutput | Output | result, retry_decision |
| GetExecutionStateInput | Input | dag_id |
| GetExecutionStateOutput | Output | dag_id, node_states, counts, paused, is_complete |
| PauseExecutionInput | Input | dag_id |
| PauseExecutionOutput | Output | dag_id, in_flight_count, pending_count, paused_at |
| ResumeExecutionInput | Input | dag_id |
| ResumeExecutionOutput | Output | dag_id, ready_count, resumed_at |
| AbortExecutionInput | Input | dag_id, reason |
| AbortExecutionOutput | Output | dag_id, completed_count, skipped_count, aborted_at |
| EvaluateRetryInput | Input | failure_context, policy, fallback_node_id |
| EvaluateRetryOutput | Output | decision, is_terminal |
| ApproveNodeInput | Input | dag_id, step_names, approver_id, authority, decision_context, token_claims_ref |
| ApproveNodeOutput | Output | dag_id, approved, not_found, still_pending, denied, denied_reasons: Vec<ApprovalDenial>, approval_records: Vec<ApprovalRecord> |

## Approval Integration (Contract Amendment)

See [approval module](./modules/approval.md) for the full contract. Summary of changes to this module:

- **`NodeStatus`** gains `AwaitingApproval` (exists in code) and **`IntentMismatch`** (new — pre-dispatch verification failure; node never dispatches; re-approval required)
- **Vocabulary note (intentional, pre-existing):** this module's `NodeStatus` (Pending/Ready/Running/…) is the **runtime executor** vocabulary; the persisted snapshot in state-persistence.md uses a different set (Pending/InProgress/…) — mapped at the state-persistence boundary (`NodeExecutionState → NodeState`). Both gain the two new variants consistently. Unifying the vocabularies is a follow-up cleanup (serialized-state migration required), out of this contract-freeze scope.
- **Dispatch choke point**: verification is inserted between `pop_dispatchable` and `execute_tool` inside `run_dispatch_loop` (the single loop used by both `execute_graph` and `resume_execution`) — every dispatch entry path is covered by one insertion

### Consequence gates: one sequence, two routes (#1074)

A dispatch refusal emits one terminal `node_failed` event before the run ends. This is the record's only terminal failure evidence when the node never started; the resume path's synthesized floor is joined by `node_id` and de-duplicated, so one attempt counts once (#1230, #1232).

`service_impl/gate.rs` owns the gate order for a node; both routes call it.

| Order | Gate | Refusal failure type |
|---|---|---|
| 1 | ADR-011 approval binding (only `requires_approval` nodes) | halt on mismatch (`IntentMismatchDetected`) |
| 2 | ADR-017 precondition (authoritative state re-read) | `precondition_denied` |
| 3 | ADR-013/ADR-015 sequence policy (prefix + companion prerequisite) | `sequence_policy_denied` / `companion_prerequisite_unmet` |
| 4 | ADR-018 capacity reservation | `capacity_denied` |
| 5 | ADR-019 effect write-ahead (record before the tool can run) | begin failure refuses the dispatch |

- `gate_node(...)` computes the verdict; `apply_gate(...)` is the dispatch-loop
  bookkeeping (record the deterministic failure, release dependents, or pause).
  Both fill loops in `run_dispatch_loop` (Phase 1 and Phase 3) share `apply_gate`,
  so their bookkeeping cannot drift.
- `execute_node` (the public single-node trait method) calls the **same**
  `gate_node` before dispatching, from the session graph it already reads. It has
  no session to pause, so `Promote`/`HaltIntent` is reported as
  `approval_required` rather than dispatched. Before #1074, `execute_node` ran a
  node with **no** approval, precondition, sequence or capacity gate: the same
  tool had two different permission answers depending on which method the caller
  used, and the difference was invisible in the signature.
- The write-ahead is the fifth gate and lives in `gate_node` (#1185). It used to
  run only in the two graph fill loops, so `execute_node` had no record and no
  fence — the same divergence one layer down. Both routes now write the record
  before the tool can run; `execute_node` settles it from the terminal outcome,
  and `spawn_concurrent_node` does the same on the graph route.
- Adding a gate means adding it to `gate_node` once. Do not re-inline a gate in
  either caller — that duplication is what produced the divergence.
- **`approve_node`** contract change: `ApproveNodeInput` gains `approver_id` (required), `authority` (optional), `decision_context` (optional), `token_claims_ref` (optional); the session's `approved: HashSet<Uuid>` becomes a map of node_id → `ApprovalRecord` (delegating to `ApprovalService`); output gains `approval_records: Vec<ApprovalRecord>`
- **Single-use semantics**: the approval is **consumed on terminal outcome** (`Pending → Consumed` on success/skipped/exhausted failure after ≥1 dispatch); failed attempts stay `Pending` so legitimate retries re-verify; non-terminal interruptions keep `Pending` for cross-process resume; TTL enforced at verification. A **refused** consumption is evidence on the run that was refused (#1263): the executor publishes a named `NodeFailed` event, moves the node result and live state from completed to failed, and skips dependents. `AlreadyConsumed` is benign only when the marker for the approval's own `(node_id, nonce)` names the settlement `(dag_id, node_id, attempt)` being recorded now — never because some session in the process consumed the node, and never because its timestamp is latest (#1267, #1271). The event carries the real attempt.
- **Retry integration**: `IntentMismatch` is non-retriable (see failure-classification) — the node stays halted for human re-approval

---

### Execution Events

| Event | Fields | When Emitted |
|-------|--------|-------------|
| ExecutionStarted | dag_id, total_nodes | execute_graph called |
| NodeExecutionStarted | dag_id, node_id, node_name, attempt | Node dispatched to worker |
| NodeExecutionCompleted | dag_id, node_id, node_name, duration_ms | Node succeeds |
| NodeExecutionFailed | dag_id, node_id, node_name, failure_type, error, retries_remaining | Node fails |
| NodeRetried | dag_id, node_id, node_name, attempt, strategy, backoff_ms | Retry triggered |
| FallbackExecuted | dag_id, original_node_id, fallback_node_id, fallback_node_name | Fallback dispatched |
| NodeSkipped | dag_id, node_id, node_name, reason | Node skipped |
| DependencyResolutionConflict | dag_id, node_id, node_name, unsatisfied_deps | Dependency issue |
| ExecutionCompleted | dag_id, total_nodes, completed/failed/skipped counts, total_duration_ms | All nodes terminal |
| ExecutionCancelled | dag_id, completed_count, remaining_count, reason | Cancellation received |
| NodeAwaitingApproval | dag_id, node_id, node_name | requires_approval node paused for sign-off |
| ApprovalRecorded | dag_id, node_id, step_name, intent_hash, approver_id, authority, decided_at | Human approved a step (bound to intent hash) |
| IntentMismatchDetected | dag_id, node_id, step_name, expected_hash, actual_hash | Pre-dispatch verification failed — HALT |
| ScopeViolationRecorded | dag_id, node_id, step_name, out_of_scope | Post-execution effects exceeded declared scope |
| NodeConsumed | dag_id, node_id, nonce | Approved node dispatched; approval consumed (single-use) |

---

### HTTP API Endpoints

| Method | Path | Request | Response | Description |
|--------|------|---------|----------|-------------|
| POST | /api/v1/execution/graphs/{id}/execute | ExecuteRequest | 202 ExecuteResponse | Start execution |
| GET | /api/v1/execution/graphs/{dag_id}/state | — | ExecutionStateResponse | Get execution state |
| GET | /api/v1/execution/graphs/{dag_id}/nodes | ?status= | NodeStatesResponse | Get per-node states |
| POST | /api/v1/execution/graphs/{dag_id}/pause | — | PauseResponse | Pause execution |
| POST | /api/v1/execution/graphs/{dag_id}/resume | — | ResumeResponse | Resume execution |
| POST | /api/v1/execution/graphs/{dag_id}/abort | AbortRequest | AbortResponse | Abort execution |
| GET | /api/v1/execution/history | ?limit=&offset= | ExecutionHistoryResponse | List execution history |
| GET | /api/v1/execution/graphs/{dag_id}/result | — | full ExecutionResult | Get completed result |
| GET | /api/v1/execution/health | — | HealthResponse | Health check |

### Error Codes

| Error Code | HTTP Status | Description |
|------------|-------------|-------------|
| EXEC_NOT_FOUND | 404 | Execution not found |
| EXEC_GRAPH_NOT_SEALED | 400 | Graph not sealed |
| EXEC_ALREADY_RUNNING | 409 | Execution in progress |
| EXEC_ALREADY_COMPLETED | 409 | Already completed |
| EXEC_NOT_RUNNING | 400 | Not running |
| EXEC_CANCELLED | 200 | Was cancelled |
| EXEC_ABORTED | 200 | Was aborted |
| EXEC_ENFORCEMENT_LIMIT | 429 | Enforcement limit exceeded |
| EXEC_INTERNAL_ERROR | 500 | Internal error |

---

## Enforcement-path cost baseline (#1208)

`engine/benches/dag_engine.rs` has an `enforcement_path` group over
`execute_graph`: `seam_absent` (the default path) and `seam_in_memory` (the
ADR-019 write-ahead + settle per node), at 1, 10, and 100 independent nodes.
Each node runs the cheapest shipped tool (`file_read` of a small repo file) so
the measurement is gate + dispatch, not tool cost. The executor is pinned to
`max_concurrent_executions: 1` to isolate marginal gate cost from tokio
scheduling; a fresh `dag_id` per iteration keeps the in-memory ledger keyed per
execution instead of hitting the retry fence.

### Baseline method, not a table

Do **not** copy absolute numbers out of this document. Criterion's default
sampling is used (3 s warm-up, 5 s measurement, 100 samples); there is no
`sample_size` override in the bench. Compare **before and after on the same
idle machine and tree**; criterion stores the baseline per group under
`target/criterion`, so the stored baseline is the regression reference:

```bash
cd engine
cargo bench --bench dag_engine -- enforcement_path
```

Run it once on the base tree, once on the changed tree, and compare the two
reports. Do **not** use `--quick`: it under-samples the group badly (measured
~44 µs vs ~198 µs for the same 1-node cell), which is a larger error than any
effect this bench is meant to detect.

### What the harness can and cannot resolve

On the machine used for #1208, the seam-absent and seam-in-memory cells overlap
at 1, 10, and 100 nodes, and the earlier 100-node delta (recorded as +26 %) did
not reproduce on a second run (+3 %, overlapping bands). **At N ≤ 100 the
in-memory seam cost is below this harness's noise floor**; the honest reading is
that no seam cost is distinguishable, not that the seam is free.

The per-node floor comes from the 100-node slope, not the 1-node cell (where
fixed setup dominates and the CI is tens of microseconds wide): roughly
170 µs/node for `file_read` + dispatch/tracing, i.e. gate overhead is not the
dominant term at these sizes. The HTTP capacity configuration is intentionally
not measured: it needs a stub transport, and adding one would benchmark the
stub's socket, not the engine.

## Module Registration

The `execution_engine` module is registered in `engine/src/lib.rs` and declares four sub-modules:

- `domain` — entities, errors, events
- `application` — service traits, DTOs, factories, implementations
- `infrastructure` — repository interfaces
- `interfaces` — HTTP API contracts

---

## Testing

45 unit tests covering:
- 28 contract compliance tests (domain entity construction, serialization, lifecycle)
- 17 implementation tests (service operations, retry decisions, factories)

Proofing scripts in `.pi/scripts/ci/`:
- `check_execution-engine_contracts.sh` — 43 contract validation points
- `check_execution-engine_coverage.sh` — 12 entities + 12 operations coverage check

CI stage: stage 25 — `execution-engine_proofing` in `run_hardening_stages.sh`

---

**Status:** Implemented  
**Last verified:** 2026-08-28 (approval integration — contract amendment)  
**Module version:** 1.1.0
