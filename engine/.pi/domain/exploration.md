---
session_id: 63c25384-1902-4b72-83bb-257f3f682af5
created: 2026-06-13
business_context: "# Rigorix Architecture Specification\n\n**Rigorix** is a **deterministic coding CLI** built in Rust. It's a **task graph compiler with execution profiles**, NOT a multi-agent system.\n\n### Core Principles\n- Template-driven: Workflows in `templates/*.toml`, not dynamic agent generation\n- DAG-based: Task nodes with dependencies, topological execution\n- Minimal LLM: LLM = planning tool only (classification, parameter extraction)\n- Bounded autonomy: Hard mathematical caps on dynamic behavior\n- Bounded retries: Max 3 retries with exponential backoff + jitter (±25%)\n- Risk-gated: Safe=auto, Medium=confirm, Dangerous=dry-run\n- Replayable: Full execution trace with state snapshots (including symbol graph)\n- Pre-validated: PlanValidator catches errors BEFORE execution\n- Auditable: Planning decisions tracked and diffable"
language: rust
group_id: com.rigorix-oss
status: draft
---

# Domain Exploration: 63c25384-1902-4b72-83bb-257f3f682af5

> **Status:** agent analysis complete — synced with rigorix core crate implementation

---

## Tech Stack

**Rust** — this project MUST be implemented with this technology.
Frame all architecture decisions, module designs, and code patterns accordingly.

---

## Business Context

**Rigorix** is a **deterministic coding CLI** built in Rust. It's a **task graph compiler with execution profiles**, NOT a multi-agent system.

### Core Principles
| Principle | Implementation |
|-----------|----------------|
| **Template-driven** | Workflows in `templates/*.toml`, not dynamic agent generation |
| **DAG-based** | Task nodes with dependencies, topological execution |
| **Minimal LLM** | LLM = planning tool only (classification, parameter extraction, template generation) |
| **Bounded autonomy** | Hard mathematical caps on dynamic behavior across 3 modes (Default, Advanced, Aggressive) |
| **Bounded retries** | Max 3 retries per node with exponential backoff + jitter (±25%) |
| **Risk-gated** | Low=auto, Medium=confirm, High=dry-run |
| **Replayable** | Full execution trace with state snapshots and event bus persistence |
| **Pre-validated** | CompositeValidator catches errors BEFORE execution |
| **Auditable** | Planning decisions tracked, diffable, and persisted via audit envelope |

### Strategic Differentiators
1. **Deterministic Planning** — Templates define structure, LLM fills parameters
2. **Bounded Execution** — Retry loops with fallbacks (no emergent chaos)
3. **Mathematical Autonomy Caps** — Hard limits via `EnforcementConfig` (3 modes: Default/Advanced/Aggressive)
4. **Symbol Graph** — Multi-language (Rust, Python, TypeScript) definitions + references with O(1) lookups
5. **Debuggable** — Event bus + replay + explain + plan diff
6. **Risk-Aware** — Tool gating by risk level via `RiskClassifier`
7. **Three Modes** — Default (template, 0 dynamic nodes), Advanced (50 dynamic), Aggressive (200 dynamic)
8. **Pre-Validation** — Catches plan errors before execution
9. **Cost Control** — LLM budget tracking with RAII reservation and hard caps
10. **Self-Extending** — TemplateGenerator creates new templates from natural language intent

---

## Actors & Roles

| Actor | Description | Interactions |
|-------|-------------|-------------|
| **Developer (User)** | Human developer invoking the Rigorix CLI to automate coding workflows | Invokes CLI commands, provides intent or template selection, reviews risk-gated confirmations/dry-runs, inspects plan diffs and execution events |
| **LLM Provider** | External AI service (Anthropic Claude, OpenAI-compatible) used for planning tasks (classification, parameter extraction, template generation) | Receives planning requests from PlanningPipeline, returns structured plan parameters or TOML templates; never executes code or tools directly |
| **PlanValidator** | Pre-execution validation gate that checks plan correctness, completeness, and risk boundaries | Receives generated plans from PlanningPipeline, validates via `CompositeValidator` against template constraints and risk rules, passes or rejects with structured errors |
| **RiskClassifier** | Classifies tools/tasks by risk level (Low, Medium, High) and enforces gating policies | Analyzes tool name and parameters; applies policy: Low=auto, Medium=confirm, High=dry-run |
| **ExecutionEnforcer** | Runtime enforcer that tracks hard caps on retries, tool calls, dynamic nodes, execution time, and LLM budget | Receives state change events from executor; enforces `EnforcementConfig` limits; emits `BudgetWarning` events |
| **TemplateGenerator** | LLM-driven component that generates new TOML workflow templates from natural language user intent + repo context when no matching template exists | Receives user intent and `RepoContext`, produces validated `Template` struct, registers it into `TemplateEngine`; consumes LLM budget via `LlmBudget` |
| **Audit System** | Records execution audit trails via typed envelopes for governance and replay | Consumes `ExecutionEvent`s from event bus; produces `AuditEnvelope` records for external audit backends |

---

## Functional Requirements

| ID | Requirement | Priority | Bounded Context |
|----|-------------|----------|----------------|
| **Template System** |
| FR-001 | The system SHALL load workflow templates from `templates/*.toml` files and validate their structure against a schema | Critical | Template System |
| FR-002 | The system SHALL provide 13 built-in templates and support loading project-local templates from `templates/` | Critical | Template System |
| **DAG Engine** |
| FR-003 | The system SHALL parse a user-provided task specification and compile it into a Directed Acyclic Graph (DAG) of task nodes with dependencies | Critical | DAG Engine |
| FR-004 | The system SHALL perform topological sorting (Kahn's algorithm) of DAG nodes to determine execution order | Critical | DAG Engine |
| FR-005 | The system SHALL support two-phase DAG construction with O(1) ready queue for scheduling | Critical | DAG Engine |
| FR-006 | The system SHALL detect cycles during DAG compilation and report the cycle path | Critical | DAG Engine |
| FR-007 | The system SHALL support per-node execution policies: max retries, retry-on failure types, retry strategy, fallback node, validation rule | High | DAG Engine |
| **Planning Pipeline** |
| FR-008 | The system SHALL submit planning requests to an LLM provider for task classification and parameter extraction only | Critical | Planning Pipeline |
| FR-009 | The system SHALL validate every plan against template constraints, completeness, and risk rules BEFORE execution begins | Critical | Planning Pipeline |
| FR-010 | The system SHALL support planning with and without graph generation (`plan` vs `plan_with_graph`) | High | Planning Pipeline |
| FR-011 | The system SHALL compute a deterministic SHA-256 planning hash from `intent + template_id + sorted parameters` | High | Planning Pipeline |
| FR-012 | The system SHALL support clarification rounds when classifier confidence < 0.7 (CLARIFICATION_THRESHOLD) | Medium | Planning Pipeline |
| FR-013 | The system SHALL support a Default Mode (template-driven, 0 dynamic nodes), Advanced Mode (50 dynamic nodes), and Aggressive Mode (200 dynamic nodes) | Medium | Enforcement |
| **Execution Engine** |
| FR-014 | The system SHALL execute DAG tasks in topological order with configurable max concurrency (ParallelExecutor with JoinSet) | Critical | Execution Engine |
| FR-015 | The system SHALL apply bounded retry logic: max 3 retries per node with exponential backoff and ±25% jitter | Critical | Execution Engine |
| FR-016 | The system SHALL support multiple retry strategies: SameOperation, ReExecute, PatchWithFeedback, Fallback, ExpandContext | Critical | Execution Engine |
| FR-017 | The system SHALL support failure classification: Transient, TestFailure, BuildFailure, LspConflict, ResourceExhausted, SystemError, NonRetryable | Critical | Failure Classification |
| **Risk Gating** |
| FR-018 | The system SHALL classify tools by risk level (Low, Medium, High) and apply gating: Low=auto, Medium=confirm, High=dry-run | Critical | Risk Gating |
| **Symbol Indexing** |
| FR-019 | The system SHALL maintain a multi-language symbol graph (Rust, Python, TypeScript) indexing definitions and references with O(1) lookup | High | Repo Engine |
| FR-020 | The system SHALL index repository symbols at execution start for enriched planning context | High | Repo Engine |
| **Event System** |
| FR-021 | The system SHALL emit all execution events to an event bus (tokio broadcast) with in-memory persistence for replay | High | Event System |
| FR-022 | The system SHALL drain persisted events into ExecutionRecord at execution end | High | Event System |
| **Cancellation** |
| FR-023 | The system SHALL support cancellation of running workflows via CancellationToken with Graceful and Immediate shutdown signals | Critical | Cancellation |
| **Template Generation** |
| FR-024 | The system SHALL generate new TOML workflow templates from natural language user intent when no matching template exists | High | Template Generation |
| FR-025 | The system SHALL analyze repository structure (directory tree, dependencies, public API, existing templates, key source files) as context for template generation | High | Template Generation |
| FR-026 | The system SHALL validate generated templates against schema and symbol graph (Phase 3 pre-generation validation) | Critical | Template Generation |
| FR-027 | The system SHALL register generated templates into the TemplateEngine at runtime for immediate use, then re-run classification | High | Template Generation |
| **Enforcement** |
| FR-028 | The system SHALL track and enforce hard caps: max retries per node (3), max total retries (10), max tool calls (100), max execution time (300s), max parallel tasks (4) in Default mode | Critical | Enforcement |
| FR-029 | The system SHALL track llmLLM calls and tokens with RAII reservation pattern, enforcing hard caps that terminate planning when exceeded | Critical | Budget Tracking |
| **State Persistence** |
| FR-030 | The system SHALL persist execution state (ExecutionState) using atomic write-rename pattern | High | State Persistence |
| FR-031 | The system SHALL track per-node state (Pending, InProgress, Completed, Failed, Skipped) with output, errors, retries, and duration | High | State Persistence |
| **Audit** |
| FR-032 | The system SHALL produce audit envelopes for all execution records with typed events for governance and replay | Medium | Audit |
| **Tool System** |
| FR-033 | The system SHALL provide sandboxed tool implementations: FileRead, FileWrite, FileAppend, FilePatch, RunCommand, LspQuery, GitRead, GitStage, GitCommit | Critical | Tool System |
| **Configuration** |
| FR-034 | The system SHALL load configuration from `rigorix.toml`, environment variables (`RIGORIX__*`), and CLI flags with layered merging | High | Configuration |
| **Error Handling** |
| FR-035 | The system SHALL use structured thiserror Error enums with full source chain tracking for all error types | Critical | Error Handling |

---

## Non-Functional Requirements

| ID | Requirement | Category | Target |
|----|-------------|----------|--------|
| NFR-001 | DAG compilation from template to executable plan SHALL complete in < 100ms for graphs up to 100 nodes | Performance | < 100ms |
| NFR-002 | Symbol graph lookups SHALL complete in O(1) time | Performance | O(1) |
| NFR-003 | Retry mechanism SHALL use exponential backoff with ±25% jitter (max 3 retries per node) | Performance | Max 3 retries |
| NFR-004 | All tool execution SHALL run in sandboxed contexts with path validation (no writes outside workspace) | Security | Path-bounded |
| NFR-005 | All LLM provider tokens and keys SHALL be wrapped in `Secret` type with redacted Debug/Display | Security | Secret wrapper |
| NFR-006 | EnforcementConfig SHALL validate against absolute safety hard-caps: max_dynamic_nodes ≤ 1000, max_time ≤ 7200s, max_parallel_tasks ≤ 64 | Security | Hard caps |
| NFR-007 | The system SHALL support cancellation within 200ms of signal receipt | Availability | < 200ms |
| NFR-008 | Event bus SHALL buffer at least 10,000 events without data loss (tokio broadcast capacity) | Scalability | 10K events |
| NFR-009 | The system SHALL produce identical execution traces for identical inputs and templates (deterministic replay via planning_hash) | Maintainability | Deterministic |
| NFR-010 | All error types SHALL implement thiserror::Error with Display source chains | Maintainability | thiserror |
| NFR-011 | Enforcement SHALL support 3 autonomy presets: Default (0 dynamic nodes, 5 LLM calls, 10K tokens), Advanced (50, 20, 100K), Aggressive (200, 50, 500K) | Scalability | 3 presets |
| NFR-012 | LLM budget SHALL use RAII reservation — unreserved budget rolls back on Drop | Performance | RAII pattern |
| NFR-013 | Template generation LLM call SHALL reserve budget before invocation and enforce hard caps | Performance | Budget reservation |
| NFR-014 | Generated template DAG SHALL be limited to a configurable max node count (default: 10) | Scalability | Max 10 nodes |
| NFR-015 | Template generation SHALL NOT allow `run_command` unless the command is on an explicit allowlist | Security | Allowlist only |
| NFR-016 | State persistence SHALL use atomic write-rename (`.tmp` → final) for crash safety | Maintainability | Atomic writes |
| NFR-017 | Event Bus SHALL persist events synchronously using `std::sync::Mutex` (not spawn) | Performance | Synchronous |

---

## Assumptions

| Assumption | Impact if Wrong | Mitigation |
|------------|----------------|-----------|
| LLM providers return structured, parseable responses for planning requests | Planning pipeline fails; user gets unusable output | Add response schema validation and retry with different formatting; fall back to manual planning |
| Workflow templates in TOML are sufficient for all coding workflows | Some complex workflows cannot be expressed, forcing Advanced Mode use | Design template schema extensibly; document template limitations in Advanced Mode docs |
| Task graphs for coding workflows are acyclic (DAG) | Cyclic dependencies break topological sort; deadlock | Detect cycles during compilation and return a descriptive error with cycle path; allow user to break cycle |
| Max 3 retries per node is sufficient for transient failures | Persistent failures exhaust retries and abort workflow | Log all attempts; suggest manual intervention after exhaustion; ensure partial results are persisted via ExecutionState |
| File system sandboxing is achievable in the target Rust runtime | Tools with sandbox escape risk compromise host system | Implement path validation in FileWriteTool; allowlist for RunCommandTool |
| The set of LLM providers and their APIs remains relatively stable | Provider API changes break planning pipeline | Abstract provider behind traits (Classifier, ParameterExtractor, TemplateGenerator); add integration tests that mock provider responses |
| LLM providers can generate valid TOML templates from natural language intent with repo context | LLM produces malformed TOML or semantically invalid templates | Post-generation validation against schema; retry with structured error feedback (up to 3 attempts); Phase 3 symbol validation |
| Three enforcement modes (Default/Advanced/Aggressive) cover all use cases | Users need intermediate or custom limits | EnforcementConfig is serializable and can be customized per-project via rigorix.toml |
| Symbol graph indexing at execution start produces fresh-enough context for planning | Stale symbols lead to incorrect plans | Index on every `run()` call; cache invalidation tracked via symbol_graph_hash in ExecutionState |
| Generated templates are bounded in complexity (3-7 nodes typical, max 10 nodes) | Generator produces overly complex templates that exceed user capacity to review | Enforce `max_nodes` config cap; show preview before registration |

---

## Bounded Contexts

| Context | Description | Entities |
|---------|-------------|----------|
| **Template System** | Manages workflow template definitions stored as TOML files. Handles parsing, schema validation, template engine (instantiation into TaskGraph), and built-in template loading. | Template, TemplateSchema, TemplateRegistry, TemplateEngine, TemplateParser, ParameterDef, RetryConfig |
| **Planning Pipeline** | Orchestrates the LLM-based planning flow: budget check, intent classification, parameter extraction, template generation fallback, TaskGraph generation, and plan validation. | Planner, PlanRequest, PlanResponse, PlanValidator, PlanningSession, PlanningResult, PlanningMetadata |
| **Template Generation** | Generates new TOML workflow templates from natural language intent + repo context when no matching template exists. Plugs into PlanningPipeline between classifier and template engine. | TemplateGenerator, RepoContext, ClaudeTemplateGenerator, OpenaiTemplateGenerator, GeneratedTemplate, GeneratorError, TemplateSummary |
| **DAG Engine** | Compiles templates into executable DAGs. Handles two-phase graph construction, topological sorting (Kahn's algorithm), cycle detection, O(1) ready queue, per-node execution policies. | TaskGraph, TaskNode, TaskEdge, TopologicalSort, CycleDetector, ExecutionPolicy, ValidationRule |
| **Execution Engine** | Executes compiled DAGs with task scheduling via tokio JoinSet, concurrency control, retry logic with backoff/jitter, and result collection. | TaskExecutor, TaskScheduler, ParallelExecutor, ExecutionPolicy, BackoffStrategy |
| **Risk Gating** | Classifies tools/tasks by risk level (Low, Medium, High) based on tool name and enforces gating policies (auto, confirm, dry-run). | RiskClassifier, RiskLevel, RiskConfig, GatePolicy, ConfirmationRequest |
| **Tool System** | Defines sandboxed tool capabilities with typed parameters, allowed operations, path validation, and execution environments. | Tool, ToolInput, ToolResult, ToolError, ToolRegistry, FileReadTool, FileWriteTool, FileAppendTool, FilePatchTool, RunCommandTool, LspQueryTool, GitReadTool, GitStageTool, GitCommitTool |
| **Repo Engine** | Multi-language code indexing and symbol graph. Indexes Rust, Python, and TypeScript files; maintains O(1) definition/reference lookups. | SymbolGraph, SymbolNode, SymbolEdge, SymbolIndex, RustIndexer, PythonIndexer, TypeScriptIndexer, SymbolDefinition |
| **Event System** | Captures all execution events as an append-only log via tokio broadcast channel with synchronous in-memory persistence. Supports subscriber fan-out and drain. | EventBus, ExecutionEvent, PersistedEvent, ConsoleEventPrinter |
| **Enforcement** | Enforces hard caps on execution behavior: retries per node, total retries, tool calls, dynamic nodes, execution time, parallel tasks. Three autonomy presets. | EnforcementConfig, ExecutionEnforcer, EnforcementPreset |
| **Budget Tracking** | Monitors and enforces LLM token and call budgets per session with RAII reservation pattern (auto-rollback on Drop) and hard caps. | LlmBudget, LlmBudgetReservation, TokenBudget, CostBudget |
| **State Persistence** | Persists execution state to disk using atomic write-rename. Tracks overall status and per-node state (Pending, InProgress, Completed, Failed, Skipped). Supports TUI graph persistence. | ExecutionState, ExecutionStatus, NodeState, NodeStatus, StateManager, ExecutionGraph, GraphManager |
| **Cancellation** | Manages graceful and immediate cancellation with CancellationToken propagation, shutdown signals, and resource cleanup. | CancellationToken, CancellationManager, ShutdownSignal, CleanupHandler |
| **Failure Classification** | Classifies execution failures into typed categories for retry routing. Maps error messages to FailureType via pattern matching. | FailureType, classify_failure, RetryStrategy |
| **Audit** | Records execution audit trails via typed envelopes for governance, replay, and external audit backends. | AuditEnvelope, AuditSender, AuditQueue |
| **Configuration** | Loads and validates configuration from `rigorix.toml`, environment variables (`RIGORIX__*`), and CLI flags with layered merging. | Config, OrchestratorConfig, LoggingConfig, ToolsConfig, EnforcementPreset, AuditConfig, LlmConfig, RiskConfig, Secret |
| **Error Handling** | Structured error types using thiserror across all modules: DagError, PlanningError, EnforcementError, LlmBudgetError, ExecutionError, ToolError, SymbolGraphError, ConfigurationError, CoreOrchestratorError. | ErrorKind, ErrorSource, ErrorChain, RecoveryStrategy |

---

## Entities

| Entity | Context | Type | Description |
|--------|---------|------|-------------|
| **Template** | Template System | Aggregate Root | A TOML file defining a workflow structure: nodes, actions, dependencies, parameters, retry config, and validation rules. Uniquely identified by kebab-case ID. |
| **TemplateEngine** | Template System | Entity | Runtime registry of loaded templates. Handles registration, lookup, and instantiation of templates into TaskGraphs with parameter substitution. |
| **TemplateParser** | Template System | Entity | Parses TOML template files into Template structs. Validates schema, action types, and parameter definitions. |
| **ParameterDef** | Template System | Value Object | A template parameter definition with name, description, type (string/path/boolean/number), required flag, and optional default value. |
| **RetryConfig** | Template System | Value Object | TOML-defined retry behavior for a template node: failure types to retry on, max attempts, backoff delay, and strategy. |
| **TaskGraph** | DAG Engine | Aggregate Root | A compiled DAG representing the executable workflow. Supports two-phase construction: add nodes, then seal and sort topologically. |
| **TaskNode** | DAG Engine | Entity | A single node in the task graph. Contains ID, name, dependencies, tool_name, tool_params, symbol_intent, allowed_tools, and ExecutionPolicy. |
| **ExecutionPolicy** | DAG Engine | Value Object | Per-node execution configuration: max_retries (default 3), retry_on failure types, retry_strategy, fallback_node, validation_rule, backoff_ms. |
| **ValidationRule** | DAG Engine | Value Object | Post-execution validation: LintPass, TestPass, TypeCheck, or Custom(command). |
| **PlanningSession** | Planning Pipeline | Aggregate Root | Tracks a complete planning lifecycle: budget check, classification, extraction (or template generation), graph generation, validation. |
| **PlanningResult** | Planning Pipeline | Value Object | Deterministic contract from planning: selected_template, parameters, confidence, requires_clarification, alternatives, planning_hash, llm_calls, tokens_used. |
| **PlanningMetadata** | Planning Pipeline | Value Object | Planning metadata for ExecutionRecord: classifier_model, extractor_model, prompt_hash, planned_at, duration_ms, llm_calls, tokens_used. |
| **UserIntent** | Core Architecture | Value Object | Raw natural language request with working directory, context key-values, and clarification history. |
| **TemplateGenerator** | Template Generation | Aggregate Root | Trait for LLM-based template generation. Implemented by ClaudeTemplateGenerator and OpenaiTemplateGenerator. |
| **RepoContext** | Template Generation | Value Object | Bundled context for generation: directory tree (2 levels), project type, existing templates, dependencies, public API, key source files, raw symbols. |
| **GeneratedTemplate** | Template Generation | Entity | A template produced by the generator, validated against schema and symbol graph, ready for registration. |
| **GeneratorError** | Template Generation | Value Object | Typed error: BudgetExhausted, LlmError, InvalidTOML, ValidationFailed, EmptyResponse, DuplicateTemplate, ContextError, Cancelled, SymbolValidation. |
| **RiskLevel** | Risk Gating | Value Object | Enum: Low (auto-execute), Medium (user confirm), High (dry-run only). Derived from tool name by RiskClassifier. |
| **RiskClassifier** | Risk Gating | Entity | Analyzes tool name and parameters to determine RiskLevel. Maps known tool names: run_command/git_commit → High, file_write/git_stage → Medium, file_read/lsp_query → Low. |
| **RiskConfig** | Risk Gating | Value Object | Configurable risk policies: auto_confirm for Low, require_review for Medium, dry_run_default for High. |
| **Tool** | Tool System | Trait | The core trait for all tools. Defines `execute(input) -> Result<ToolResult, ToolError>`. Implemented by each tool type. |
| **ToolRegistry** | Tool System | Entity | Registry of all available tools by name. Supports registration and lookup. |
| **ToolInput** | Tool System | Value Object | JSON parameters passed to a tool's execute method. |
| **ToolResult** | Tool System | Value Object | Tool output with human-readable text, optional exit_code, and side_effects vector. |
| **FileReadTool** | Tool System | Entity | Low-risk tool for reading file contents from disk. |
| **FileWriteTool** | Tool System | Entity | Medium-risk tool for writing/overwriting files with atomic write-rename pattern. |
| **FileAppendTool** | Tool System | Entity | Medium-risk tool for appending content to existing files. |
| **FilePatchTool** | Tool System | Entity | Medium-risk tool for AST-aware file patching with anchor string positioning. |
| **RunCommandTool** | Tool System | Entity | High-risk tool for executing shell commands via tokio process, with path confinement. |
| **LspQueryTool** | Tool System | Entity | Low-risk tool for querying language server for type information. |
| **GitReadTool** | Tool System | Entity | Low-risk tool for reading git log/diff output. |
| **GitStageTool** | Tool System | Entity | Medium-risk tool for staging files in git. |
| **GitCommitTool** | Tool System | Entity | High-risk tool for creating git commits. |
| **SymbolGraph** | Repo Engine | Aggregate Root | In-memory graph of all indexed code symbols across multiple languages. Supports O(1) definition lookup and reference traversal. |
| **SymbolNode** | Repo Engine | Entity | A single symbol in the graph with name, kind, file location, and signature. |
| **SymbolDefinition** | Repo Engine | Entity | A definition symbol with full source text, documentation, and source files set. |
| **RustIndexer** | Repo Engine | Entity | Indexes Rust source files using tree-sitter-rust. |
| **EventBus** | Event System | Aggregate Root | Central pub-sub event bus backed by tokio broadcast. Events are persisted synchronously in-memory. Supports drain at execution end. |
| **ExecutionEvent** | Event System | Value Object | Tagged union of all observable events: PlanningStarted, PlanningCompleted, NodeStarted, NodeCompleted, NodeFailed, NodeRetrying, ToolExecuted, ExecutionCompleted, ExecutionFailed, ExecutionCancelled, BudgetWarning. |
| **PersistedEvent** | Event System | Value Object | An event with a monotonic sequence number for ordered replay. |
| **EnforcementConfig** | Enforcement | Value Object | Hard cap configuration: max_retries_per_node (3), max_total_retries (10), max_time_seconds (300), max_tool_calls (100), max_dynamic_nodes (0/50/200), max_parallel_tasks (4/8/16), max_llm_calls (5/20/50), max_llm_tokens (10K/100K/500K). |
| **ExecutionEnforcer** | Enforcement | Entity | Atomic runtime enforcer tracking tool_calls, dynamic_nodes, total_retries, per-node retries, and time limit. Validates all actions against EnforcementConfig. |
| **EnforcementPreset** | Configuration | Value Object | Enum selecting autonomy mode: Default, Advanced, Aggressive. |
| **LlmBudget** | Budget Tracking | Entity | Tracks token count and LLM call count. Supports RAII reservation with auto-rollback on Drop. Has cancellation token for coordinated shutdown. |
| **LlmBudgetReservation** | Budget Tracking | Value Object | RAII guard that holds budget capacity. Commits or auto-rolls back on Drop. |
| **ExecutionState** | State Persistence | Aggregate Root | Serialized snapshot of an execution: status, start/completed time, per-node state map (IndexMap), symbol_graph_hash. Persisted via StateManager. |
| **NodeState** | State Persistence | Entity | Per-node execution state: node_id, status (Pending/InProgress/Completed/Failed/Skipped), output, error, retries, duration_ms. |
| **StateManager** | State Persistence | Entity | Manages atomic persistence of ExecutionState to disk. Uses write-rename pattern with fd-lock cross-process locking. |
| **ExecutionGraph** | State Persistence | Entity | Graph structure for TUI visualization of past executions. Persisted via GraphManager. |
| **CancellationToken** | Cancellation | Value Object | Tokio-util CancellationToken that signals cancellation when triggered. Propagated to all concurrent tasks. |
| **CancellationManager** | Cancellation | Entity | Manages cancellation with Graceful and Immediate ShutdownSignal levels. Supports watch channel for signal subscribers. |
| **ShutdownSignal** | Cancellation | Value Object | Enum: Graceful (let running tasks finish), Immediate (abort all in-flight work). |
| **FailureType** | Failure Classification | Value Object | Enum: Transient, TestFailure, BuildFailure, LspConflict, ResourceExhausted, SystemError, NonRetryable. Each maps to a RetryStrategy. |
| **RetryStrategy** | Failure Classification | Value Object | Enum: SameOperation, ReExecute, PatchWithFeedback, Fallback, ExpandContext. |
| **Config** | Configuration | Aggregate Root | Full application config with sub-configs: orchestrator, logging, tools (risk), enforcement preset, audit, llm (provider, model, api_key, base_url). |
| **Secret** | Configuration | Value Object | API key wrapper with redacted Debug/Display/Serialize output. Accessible only via `.expose()`. |
| **AuditEnvelope** | Audit | Entity | Typed envelope for execution audit records. Contains execution metadata, events, and HMAC signature for integrity. |
| **CoreOrchestratorError** | Error Handling | Value Object | Root error type wrapping all domain-specific errors via `#[from]`. Includes DagError, PlanningError, EnforcementError, LlmBudgetError, ExecutionError, ToolError, SymbolGraphError, ConfigurationError, and Cancelled/Io/Json/Http variants. |

---

## Domain Events

| Event | Context | Description | Triggered By |
|-------|---------|-------------|-------------|
| **PlanningStarted** | Event System | The planning pipeline has started processing a user intent | Orchestrator::run() begins |
| **PlanningCompleted** | Event System | Classification succeeded; a template was selected with parameters | PlanningPipeline completes with sufficient confidence |
| **NodeStarted** | Event System | A DAG node has started execution | ParallelExecutor dequeues a ready node |
| **NodeCompleted** | Event System | A node completed successfully with output and duration | TaskExecutor finishes without error |
| **NodeFailed** | Event System | A node failed with an error message and attempt number | Task throws error (may still have retries remaining) |
| **NodeRetrying** | Event System | A node is being retried after a failure, with calculated delay | Failure occurs and retry is permitted by ExecutionEnforcer |
| **ToolExecuted** | Event System | A tool was invoked (or skipped in dry-run mode) with risk level | TaskExecutor invokes a Tool through ToolRegistry |
| **ExecutionCompleted** | Event System | The full execution completed successfully | All DAG nodes complete without fatal errors |
| **ExecutionFailed** | Event System | The execution failed with a top-level error | Fatal error during planning, validation, or execution |
| **ExecutionCancelled** | Event System | The execution was cancelled by user or signal | User sends SIGINT/SIGTERM or TUI cancel command |
| **BudgetWarning** | Event System | An LLM or enforcement budget is approaching its limit | Budget usage crosses configurable warning threshold |
| **BudgetExceeded** | Budget Tracking | LLM token or cost budget has been exhausted | LlmBudget.reserve() fails due to max_calls or max_tokens |
| **DAGCompiled** | DAG Engine | A template has been successfully compiled into a TaskGraph | TemplateEngine.generate() succeeds with topological sort |
| **CycleDetected** | DAG Engine | A cycle was detected during DAG compilation | CycleDetector finds a cycle; compilation fails |
| **TemplateGenerated** | Template Generation | A new template was successfully produced by the LLM generator and validated | Generator.generate() returns a valid Template |
| **TemplateGenerationFailed** | Template Generation | Template generation failed (budget, parse, validation, or provider error) | Generator returns GeneratorError variant |
| **TemplateRegistered** | Template Generation | A generated template was registered into the TemplateEngine | TemplateEngine.register() succeeds |
| **StatePersisted** | State Persistence | Execution state (ExecutionState) was saved to disk | StateManager.save_state() after state transition |
| **SymbolIndexed** | Repo Engine | Code symbols were indexed and added to the symbol graph | RustIndexer/PythonIndexer/TypeScriptIndexer runs at execution start |
| **ToolCallRecorded** | Enforcement | A tool call was recorded by the ExecutionEnforcer | ExecutionEnforcer.record_tool_call() increments counter |
| **RetryRecorded** | Enforcement | A retry attempt was recorded by the ExecutionEnforcer | ExecutionEnforcer.record_retry() succeeds |
| **PlanValidated** | Planning Pipeline | A plan passed all validation checks | CompositeValidator.validate() returns is_valid=true |
| **PlanRejected** | Planning Pipeline | A plan failed validation with structured error details | CompositeValidator.validate() returns is_valid=false |

---

## Ubiquitous Language

| Term | Definition | Bounded Context | Aliases/Synonyms |
|------|-----------|----------------|-----------------|
| **Template** | A TOML file defining a workflow structure: nodes, actions, dependencies, parameters, retry config, and validation rules | Template System | workflow-definition, blueprint |
| **TemplateEngine** | Runtime registry that loads, registers, and instantiates templates into TaskGraphs with parameter substitution | Template System | template-registry, template-runtime |
| **TaskGraph** | A compiled Directed Acyclic Graph of task nodes with dependency edges and topological ordering | DAG Engine | workflow-graph, plan-graph, execution-dag |
| **TaskNode** | A single node in the TaskGraph with ID, name, tool binding, parameters, dependencies, and execution policy | DAG Engine | task, node, work-item |
| **ExecutionPolicy** | Per-node configuration: max_retries, retry_on failure types, retry_strategy, fallback_node, validation_rule, backoff_ms | DAG Engine | node-policy, retry-config |
| **FailureType** | Classification of execution failures for retry routing: Transient, TestFailure, BuildFailure, LspConflict, ResourceExhausted, SystemError, NonRetryable | Failure Classification | failure-class, error-type |
| **RetryStrategy** | Strategy for retrying failed operations: SameOperation, ReExecute, PatchWithFeedback, Fallback, ExpandContext | Execution Engine | retry-strategy, recovery-strategy |
| **Plan** | A validated, executable mapping from template structure to concrete task nodes with resolved parameters | Planning Pipeline | execution-plan, workflow-plan |
| **PlanningResult** | Deterministic contract from planning phase: selected_template, parameters, confidence, planning_hash, llm_calls, tokens_used | Planning Pipeline | plan-result, planning-output |
| **PlanningHash** | SHA-256 hash of `intent + template_id + sorted parameters` for deterministic replay comparison | Planning Pipeline | plan-hash, execution-digest |
| **CLARIFICATION_THRESHOLD** | Confidence threshold (0.7) below which the classifier requires clarification or triggers template generation | Planning Pipeline | low-confidence-threshold, gen-trigger |
| **UserIntent** | Raw natural language request from the user with working directory, context, and clarification history | Core Architecture | user-request, intent, query |
| **PlanValidator** | A gate that verifies plan correctness, completeness, and risk boundary compliance before execution | Planning Pipeline | preflight-checker, plan-checker |
| **CompositeValidator** | Aggregation of multiple PlanValidator implementations for combined validation | Planning Pipeline | validator-chain, multi-validator |
| **RiskLevel** | Enumeration of execution risk: Low (auto), Medium (confirm), High (dry-run) | Risk Gating | risk-class, danger-level |
| **RiskClassifier** | Component that maps tool names to RiskLevel (run_command→High, file_write→Medium, file_read→Low) | Risk Gating | risk-analyzer, tool-scorer |
| **RiskConfig** | Configurable gating policies: auto_confirm, require_review, dry_run_default | Risk Gating | gate-config, risk-rule |
| **SymbolGraph** | In-memory multi-language graph of code symbols with O(1) definition lookups and reference traversal | Repo Engine | code-graph, symbol-index, definition-graph |
| **ExecutionEnforcer** | Runtime component tracking hard caps on retries, tool calls, dynamic nodes, execution time | Enforcement | enforcer, limit-tracker, cap-enforcer |
| **EnforcementConfig** | Configuration of hard caps across 3 presets: Default, Advanced, Aggressive | Enforcement | enforcement-settings, autonomy-config |
| **LlmBudget** | RAII-managed budget for LLM calls and tokens with auto-rollback on Drop | Budget Tracking | token-budget, cost-tracker, usage-quota |
| **LlmBudgetReservation** | RAII guard that holds budget capacity; commits actual tokens or auto-rolls back | Budget Tracking | budget-reservation, token-hold |
| **EventBus** | Central pub-sub event bus (tokio broadcast) with synchronous in-memory persistence | Event System | event-channel, message-bus, event-stream |
| **ExecutionEvent** | Tagged union of all observable events during a run: PlanningStarted, NodeStarted, ToolExecuted, etc. | Event System | domain-event, notification, signal |
| **PersistedEvent** | An event with monotonic sequence number for ordered replay | Event System | sequenced-event, replay-event |
| **ExecutionState** | Serializable snapshot of an execution: status, node states, symbol_graph_hash | State Persistence | run-state, execution-snapshot |
| **NodeState** | Per-node execution state: status (Pending/InProgress/Completed/Failed/Skipped), output, error, retries, duration | State Persistence | node-status, task-state |
| **StateManager** | Persistence manager using atomic write-rename pattern with fd-lock | State Persistence | state-persister, execution-store |
| **CancellationToken** | A token that signals cancellation to all concurrent tasks | Cancellation | cancel-token, abort-signal |
| **CancellationManager** | Manages graceful and immediate shutdown with watch channel subscribers | Cancellation | cancel-manager, shutdown-controller |
| **ShutdownSignal** | Enum: Graceful (finish in-flight) or Immediate (abort now) | Cancellation | shutdown-level, abort-mode |
| **Secret** | API key wrapper with redacted Debug/Display/Serialize; accessible only via `.expose()` | Configuration | api-key, credential, sensitive-value |
| **TemplateGenerator** | Trait for LLM-based template creation from natural language intent + repo context | Template Generation | template-creator, workflow-generator |
| **RepoContext** | Repository snapshot for generation: dir_tree, project_type, dependencies, public_api, key_files, symbols | Template Generation | repo-metadata, workspace-context |
| **ClaudeTemplateGenerator** | TemplateGenerator implementation using Anthropic Messages API | Template Generation | claude-generator, anthropic-generator |
| **OpenaiTemplateGenerator** | TemplateGenerator implementation using OpenAI-compatible API | Template Generation | openai-generator, compatible-generator |
| **BoundedAutonomy** | Design principle that caps dynamic behavior with hard mathematical limits across 3 modes | Core Architecture | autonomy-cap, bounded-control |
| **TopologicalSort** | Kahn's algorithm ordering of TaskNodes ensuring dependencies execute before dependents | DAG Engine | topo-sort, dependency-order |
| **PlanDiff** | A structured comparison between two planning decisions for auditing | Event System | plan-comparison, audit-diff |
| **ExecutionRecord** | Complete record of an execution: context, planning metadata, events, task results | State Persistence | run-record, execution-log |
| **ParallelExecutor** | DAG executor using tokio JoinSet for parallel task execution with concurrency control | Execution Engine | dag-executor, task-runner |
| **AuditEnvelope** | Typed envelope containing execution audit data with HMAC integrity | Audit | audit-record, governance-envelope |
| **GeneratorError** | Typed error for generation failures: BudgetExhausted, LlmError, InvalidTOML, ValidationFailed, etc. | Template Generation | generation-failure, gen-error |
| **Dogfooding** | Testing Rigorix by using Rigorix to build Rigorix features (self-hosting validation) | Core Architecture | self-hosting, eat-your-own-dogfood |

---

## Open Questions

1. How should the system handle LLM provider API changes that break the planning response schema? Should there be a version negotiation protocol or a migration strategy?
2. What is the exact CLARIFICATION_THRESHOLD value? Actual code uses 0.7 — should this be configurable per user or hard-coded?
3. Should generated templates be automatically persisted to disk for reuse across sessions, or only held in the runtime registry?
4. How should the template generator handle LLM responses that produce structurally valid but semantically nonsensical templates (e.g., a template that compiles but doesn't fulfill the intent)?
5. Should the generator support iterative refinement — allowing the user to tweak a generated template via natural language feedback?
6. What commands/patterns belong on the `run_command` allowlist? Should it be per-project configurable?
7. How does the dogfooding test verify that Rigorix built the template generator feature *under its own governance rules* (deterministic, bounded, auditable)?
8. Should the EventBus support persistence to disk (SQLite or append-only log) for replay across process restarts, or is in-memory sufficient?
9. Is the three-mode enforcement system (Default/Advanced/Aggressive) sufficient, or should users be able to set custom limits via rigorix.toml?

---

## Aggregate Roots

| Aggregate Root | Bounded Context | Key Entities | Invariants |
|----------------|----------------|-------------|------------|
| **Template** | Template System | Template, TemplateEngine, TemplateParser | Template TOML must validate against schema; template IDs must be unique within registry; parameter defaults must match declared types |
| **TaskGraph** | DAG Engine | TaskGraph, TaskNode, ExecutionPolicy | Graph must be acyclic (no cycles); every dependency edge must reference valid source/target nodes; topological ordering must be total |
| **PlanningSession** | Planning Pipeline | PlanningResult, PlanningMetadata, UserIntent | Session must complete with validated plan or explicit clarification; budget must not exceed configured hard cap |
| **TemplateGenerator** | Template Generation | TemplateGenerator, RepoContext, GeneratedTemplate, GeneratorError | Generated template must pass validate_template() before registration; generator must reserve LLM budget before invocation; generated ID must not conflict with existing templates; generated template must pass symbol graph validation |
| **SymbolGraph** | Repo Engine | SymbolGraph, SymbolNode, SymbolDefinition | Every definition must have O(1) lookup by name; no duplicate definitions; multi-language symbols coexist in one graph |
| **EventBus** | Event System | EventBus, ExecutionEvent, PersistedEvent | Events are append-only in sequence order; persisted events can be drained once at execution end; broadcast subscribers see all events after subscription |
| **EnforcementConfig** | Enforcement | EnforcementConfig, ExecutionEnforcer | Hard caps must validate against absolute safety limits (max_dynamic_nodes ≤ 1000, max_time ≤ 7200s, max_parallel_tasks ≤ 64); atomic counters must be thread-safe |
| **ExecutionState** | State Persistence | ExecutionState, NodeState, StateManager | State persists via atomic write-rename (crash-safe); node IDs must match TaskGraph nodes; status transitions must be valid |
| **Config** | Configuration | Config, Secret, RiskConfig, LlmConfig | Config must merge from multiple sources (file, env, CLI); provider keys must use Secret wrapper with redacted output |
