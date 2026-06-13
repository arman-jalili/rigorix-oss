# Implementation Roadmap

> **Canonical Reference:** `.pi/architecture/implementation-roadmap.md`
> **Source:** Module dependency analysis from architecture scaffold (Session 63c25384)
> **Last Updated:** 2026-06-13

---

## Overview

Rigorix consists of **17 modules** across **5 implementation phases**. Each phase builds on the previous, with clear milestone checkpoints to validate progress.

---

## Phase 0: Foundation

*No internal dependencies — build these first.*

| # | Module | Key Deliverables | Depends On |
|---|--------|-----------------|------------|
| 1 | **Configuration** | `Config` struct, multi-source loading (`rigorix.toml` + env + CLI), `Secret` wrapper, `EnforcementPreset`, `RiskConfig`, `LlmConfig` | — |
| 2 | **Error Handling** | `CoreOrchestratorError` root enum, `DagError`, `PlanningError`, `EnforcementError`, `LlmBudgetError`, `ExecutionError`, `ToolError`, `SymbolGraphError`, `ConfigurationError`; all `#[from]` conversions; `reqwest::Error → Http` conversion | — |
| 3 | **Cancellation** | `CancellationToken` (tokio-util), `CancellationManager`, `ShutdownSignal` (Graceful/Immediate), watch channel for subscribers | — |
| 4 | **Failure Classification** | `FailureType` enum (7 variants), `classify_failure()` pattern-matching function, `RetryStrategy` enum (5 variants), `FailureType → RetryStrategy` mapping | — |

**Validation Gate:** `cargo test` passes with all error types, cancellation tokens, and failure classification.

---

## Phase 1: Infrastructure

*Depends on Phase 0.*

| # | Module | Key Deliverables | Depends On |
|---|--------|-----------------|------------|
| 5 | **Event System** | `EventBus` with `tokio::sync::broadcast`, synchronous `Mutex<Vec<PersistedEvent>>`, 11 `ExecutionEvent` variants, `ConsoleEventPrinter`, `drain_persisted()` | 2 (Error Handling) |
| 6 | **Enforcement** | `EnforcementConfig` with 3 presets (Default/Advanced/Aggressive), `ExecutionEnforcer` with atomic counters, `validate()` against safety hard-caps | 2 (Error Handling), 1 (Configuration) |
| 7 | **Budget Tracking** | `LlmBudget` with RAII `reserve()`/`commit()`, `LlmBudgetReservation` with auto-rollback on Drop, `CancellationToken` integration | 3 (Cancellation), 2 (Error Handling), 1 (Configuration) |
| 8 | **State Persistence** | `ExecutionState`, `NodeState`, `NodeStatus`, `ExecutionStatus`, `StateManager` with atomic write-rename, `fd-lock` cross-process locking, `ExecutionGraph` + `GraphManager` | 2 (Error Handling), 1 (Configuration) |
| 9 | **Risk Gating** | `RiskLevel` enum (Low/Medium/High), `RiskClassifier` (tool name → level mapping), `RiskConfig` with per-tool overrides | 1 (Configuration) |

**Validation Gate:** Event bus emits/receives events; enforcement rejects over-limit actions; budget reserves/commits/rolls back correctly; state persists atomically.

---

## Phase 2: Domain Logic

*Depends on Phase 0–1.*

| # | Module | Key Deliverables | Depends On |
|---|--------|-----------------|------------|
| 10 | **Template System** | `TemplateParser` (TOML deserialization), `TemplateEngine` (register + generate), `Template`, `TemplateNode`, `TemplateAction`, `ParameterDef`, 13 built-in templates, `validate_template()` | 2 (Error Handling), 1 (Configuration) |
| 11 | **DAG Engine** | `TaskGraph` with two-phase construction, Kahn's algorithm `topological_sort()`, `CycleDetector`, O(1) ready queue, `ExecutionPolicy`, `ValidationRule`, `PlanDiff`, `ImpactLevel` | 4 (Failure Classification), 2 (Error Handling) |
| 12 | **Tool System** | `Tool` trait, `ToolRegistry`, `ToolInput`, `ToolResult`, `FileReadTool`, `FileWriteTool` (atomic), `FileAppendTool`, `FilePatchTool` (AST-aware), `RunCommandTool` (allowlisted), `LspQueryTool`, `GitReadTool`, `GitStageTool`, `GitCommitTool`, `execute_with_risk_gate()` | 9 (Risk Gating), 1 (Configuration), 2 (Error Handling) |
| 13 | **Repo Engine** | `SymbolGraph` with O(1) lookup, `SymbolDefinition`, `RustIndexer` (tree-sitter-rust), `PythonIndexer` (tree-sitter-python), `TypeScriptIndexer` (tree-sitter-typescript), `SymbolWorkspaceIntent` | 2 (Error Handling), external: tree-sitter crates |

**Validation Gate:** Templates parse and generate TaskGraphs; DAG sorts topologically and detects cycles; tools execute with risk gating; symbol graph indexes code.

---

## Phase 3: Orchestration

*Depends on Phase 2.*

| # | Module | Key Deliverables | Depends On |
|---|--------|-----------------|------------|
| 14 | **Planning Pipeline** | `PlanningPipeline` struct, 6-phase flow (budget → classify → extract → generate → validate → hash), `Classifier` trait + `ClaudeClassifier` + `OpenaiClassifier` + `MockClassifier`, `ParameterExtractor` trait + implementations, `PlanningResult` + `PlanningMetadata`, `CLARIFICATION_THRESHOLD = 0.7`, deterministic `planning_hash` | 10 (Template System), 7 (Budget Tracking), 13 (Repo Engine), 2 (Error Handling), 5 (Event System) |
| 15 | **Template Generation** | `TemplateGenerator` trait, `ClaudeTemplateGenerator`, `OpenaiTemplateGenerator`, `MockGenerator`, `RepoContext` with dir_tree/dependencies/public_api/key_files, Phase 3 symbol validation, LLM retry loop (≤3 attempts) | 14 (Planning Pipeline), 10 (Template System), 13 (Repo Engine), 7 (Budget Tracking), 2 (Error Handling) |
| 16 | **Execution Engine** | `ParallelExecutor` with tokio `JoinSet`, per-node retry loop with `calculate_backoff()`, `ExecutionEnforcer` integration, `CancellationToken` propagation, `ExecutionEvent` emission, `TaskResult` collection | 11 (DAG Engine), 3 (Cancellation), 6 (Enforcement), 9 (Risk Gating), 12 (Tool System), 4 (Failure Classification), 5 (Event System), 8 (State Persistence) |

**Validation Gate:** `rigorix plan "add feature"` produces a validated plan; `rigorix run "add feature"` executes end-to-end with retry, cancellation, and state persistence.

---

## Phase 4: Observability

*Depends on Phase 3.*

| # | Module | Key Deliverables | Depends On |
|---|--------|-----------------|------------|
| 17 | **Audit** | `AuditEnvelope` with HMAC signature, `AuditSender` with retry logic, `AuditQueue` for failed deliveries, `CircuitBreaker` (open/half-open/closed) | 5 (Event System), 1 (Configuration), 2 (Error Handling) |

**Validation Gate:** Full execution produces an audit envelope; failed deliveries queue and retry; circuit breaker protects against backend failures.

---

## Milestone Checkpoints

| Milestone | Modules Complete | What Works | Test Command |
|-----------|-----------------|------------|--------------|
| **M0: Foundation** | #1–4 | Config loads, errors propagate, cancellation signals work, failures classify | `cargo test` |
| **M1: Infrastructure** | #5–9 | Events flow, enforcement gates actions, budget tracks LLM, state persists, risk classifies tools | `cargo test --test integration` |
| **M2: Domain Logic** | #10–13 | Templates parse + generate, DAGs compile + sort, tools execute safely, code indexes | `rigorix template list` |
| **M3: Orchestration** | #14–16 | Full planning → execution pipeline | `rigorix run "read src/lib.rs"` |
| **M4: Observability** | #17 | Complete audit trail | `rigorix history` |

---

## Dependency Graph (visual)

```
Phase 0              Phase 1              Phase 2               Phase 3              Phase 4
───────              ───────              ───────               ───────              ───────

1. Configuration ──► 5. Event System
                     6. Enforcement ──► 11. DAG Engine ──► 16. Execution Engine ──► 17. Audit
2. Error Handling    7. Budget Track ──► 14. Planning Pipeline ──► 15. Template Gen
3. Cancellation      8. State Persist   12. Tool System ──────┘
4. Failure Classify  9. Risk Gating ──► 13. Repo Engine ──┘
                                       10. Template System ─┘
```

---

## Risk Assessment

| Risk | Phase | Mitigation |
|------|-------|------------|
| LLM provider API changes break planning | P3 | Trait-based abstraction; `MockClassifier`/`MockGenerator` for offline mode |
| tree-sitter grammar version conflicts | P2 | Pin versions in Cargo.toml; vendored build if needed |
| DAG execution performance with 100+ nodes | P3 | Benchmark at `dag_bench.rs`; O(1) ready queue design |
| Template generation quality too low | P3 | 3-attempt retry loop with LLM feedback; Phase 3 symbol validation catches hallucinations |
| Cross-platform file locking (fd-lock) | P1 | Test on Linux/macOS/Windows CI |

---

## Key Architecture Principles to Preserve

1. **No anyhow in library code** — all errors via `thiserror`
2. **RAII budget reservation** — every LLM call through `LlmBudget::reserve()`
3. **Atomic write-rename** — all state persistence uses `.tmp` → `rename` pattern
4. **Risk gating** — every tool execution through `execute_with_risk_gate()`
5. **Deterministic planning_hash** — SHA-256 of `intent + template_id + sorted params`
6. **Event-driven observability** — every state change published to `EventBus`
