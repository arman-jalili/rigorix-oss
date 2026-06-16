# Ubiquitous Language

> Canonical glossary for **rigorix-oss**.
> All code MUST use these terms. Aliases/synonyms listed below are **prohibited** in source identifiers.
> Drift is detected by `.pi/scripts/validate-ubiquitous-language.sh`.

## Glossary

| Term | Definition | Bounded Context | Aliases/Synonyms | Examples |
|------|-----------|----------------|-----------------|---------|
| AtomicWriteRename | Crash-safe file writing pattern: write to temp file, fsync, rename over target. Used for state persistence. | StatePersistence | atomic-save, safe-write | `write(tmp) -> fsync -> rename(tmp, target)` |
| AuditEnvelope | Typed governance-grade audit record containing execution_id, planning_hash, and ordered event sequence for integrity verification. | Audit | audit-record, audit-packet | `AuditEnvelope { execution_id, planning_hash, events }` |
| BackoffStrategy | Strategy for computing delay between retry attempts. Variants: Fixed, Exponential, Linear, Immediate. | ExecutionEngine | backoff, delay-strategy | `BackoffStrategy::Exponential { multiplier: 2.0 }` |
| BoundedAutonomy | Design principle: the execution engine has hard caps on dynamic behavior (retries, tokens, tool calls) rather than unbounded LLM-driven decisions. | Configuration, Enforcement | — | `max_retries: 3`, `max_tool_calls: 50` |
| BudgetTracking | Context that monitors and enforces LLM token and call budgets with RAII reservation pattern. | BudgetTracking | budget-monitoring, resource-budgeting | `LlmBudget::reserve(1000)?` |
| CancellationService | Manages two-level shutdown signals: Graceful (finish in-flight) and Immediate (abort all). | Cancellation | shutdown-handler, abort-service | Ctrl+C = Graceful, Ctrl+C twice = Immediate |
| ClassificationResult | LLM output from intent classification: matched template ID, confidence score (0.0–1.0), and extracted parameters. | PlanningPipeline | classification-output | `ClassificationResult { template_id: "build", confidence: 0.95 }` |
| CleanArchitecture | Architecture pattern with domain/application/infrastructure/interfaces layers enforcing inward dependency rule. Followed by every engine module. | All | hexagonal-architecture | Each engine module follows this layering structure |
| CliBoundary | The outermost shell of the system: command parsing, TUI rendering, process lifecycle wrapping the engine library. | CLI | shell, interface-layer | `rigorix run "add auth middleware"` |
| CliCommand | Parsed CLI command variant: Run, Plan, Init, Template, Audit, History, Logs. | CLI | command, subcommand | `CliCommand::Run { intent: "..." }` |
| Config | Multi-source merged configuration (rigorix.toml + env vars + CLI flags). Root aggregate for the Configuration context. | Configuration | settings, rigorix-config | Loaded at startup via `ConfigService::load()` |
| ContractFreeze | Design phase where interfaces are defined and frozen before implementation. All engine modules follow this to ensure stability. | All | — | `# Contract (Frozen)` annotations in engine source code |
| DAG | Directed Acyclic Graph of task nodes with dependency edges. The core execution structure. | DAGEngine | TaskGraph, workflow-graph | `TaskGraph { nodes: [...], sealed: true }` |
| EnforcementConfig | Configuration for execution safety limits: resource budgets, tool call policies, execution hard caps. | Enforcement | limits-config, safety-config | `EnforcementConfig { max_tokens: 100000, max_tool_calls: 50 }` |
| EventBus | Central pub-sub channel using tokio broadcast with in-memory append-only log. Supports subscriber fan-out for TUI, console, and persistence. | EventSystem | event-channel, bus | `EventBus::publish(event)` → all subscribers receive it |
| ExecutionEvent | One of 11 typed lifecycle events emitted during a Rigorix run (PlanningStarted, PlanningCompleted, NodeStarted, NodeCompleted, NodeFailed, NodeRetrying, ToolExecuted, ExecutionCompleted, ExecutionFailed, ExecutionCancelled, BudgetWarning). | EventSystem | lifecycle-event, telemetry-event | `ExecutionEvent::NodeStarted { node_id, node_name }` |
| ExecutionEnforcer | Safety gate that checks budgets, risk levels, and tool policies before allowing every tool call. Central authority for enforcement decisions. | Enforcement | enforcer, gatekeeper | `Enforcer::check(tool, risk_level) -> Allow | Block | Confirm` |
| ExecutionPolicy | Per-node configuration for retry behavior, fallback behavior, and post-execution validation. | DAGEngine | node-policy, retry-config | `ExecutionPolicy { max_retries: 3, backoff_ms: 100 }` |
| ExecutionResult | Aggregate result of a full DAG execution: per-node TaskResults, summary counts, timing, retry statistics. | ExecutionEngine | execution-output, run-result | `ExecutionResult { completed: 5, failed: 0, skipped: 1 }` |
| ExecutionSession | A single CLI-managed execution run linking engine execution_id to CLI session metadata. | CLI | session, run | Created on `rigorix run` |
| ExecutionState | Serializable snapshot of overall execution status (Pending, Running, Completed, Failed, Cancelled) and per-node states. | StatePersistence | state-snapshot, persistence-state | Persisted via atomic write-rename |
| FailureContext | Contextual information about a node failure: failure type, error message, attempt count, timing, and previous errors. | ExecutionEngine | failure-details, error-context | `FailureContext { failure_type: "transient", attempt: 2 }` |
| FailureType | Categorized failure variant: Transient, LspConflict, CompileError, TestFailure, MissingDependency, PlanConflict, Permanent, Unknown. | FailureClassification | failure-category | `FailureType::Transient` |
| GracefulShutdown | Cancellation level: finish in-flight node execution, then stop. | Cancellation | soft-cancel | Single Ctrl+C |
| ImmediateShutdown | Cancellation level: abort all in-flight work immediately. | Cancellation | hard-abort, kill | Double Ctrl+C |
| KahnAlgorithm | Topological sort algorithm used for cycle detection and ordering in the DAG. | DAGEngine | — | Used in `TaskGraph::seal()` |
| LlmBudget | Aggregate root for token and call budget tracking. Emits warnings at configurable thresholds and enforces hard limits. | BudgetTracking | token-budget, call-budget | `LlmBudget { max_tokens: 100000, used_tokens: 45000 }` |
| LlmBudgetReservation | RAII guard: reserves budget on creation, auto-returns on Drop. Ensures atomic budget consumption. | BudgetTracking | reservation-guard, raii-guard | `let _guard = budget.reserve(1000)?;` |
| NodeExecutionState | Runtime lifecycle tracker for a single node: status transitions (Pending -> Ready -> Running -> Terminal), retry counts, timing. | ExecutionEngine | node-state, node-tracker | `NodeExecutionState { status: Running, retry_attempts: 2 }` |
| NodeStatus | Lifecycle status enum: Pending, Ready, Running, Completed, Failed, Skipped. | ExecutionEngine | node-status, status | `NodeStatus::Running` |
| ParallelExecutorConfig | Configuration for the parallel executor: max concurrency, default retry policy, cancellation/enforcement toggles. | ExecutionEngine | executor-config, concurrency-config | `ParallelExecutorConfig { max_concurrent_executions: 4 }` |
| ParameterDef | Schema for a template parameter: name, type, default value, required flag, human-readable description. | Templates | param-schema, parameter-schema | `ParameterDef { name: "module", type: "string", required: true }` |
| PlanDiff | Structural diff between two execution plans: added/removed/changed nodes, policy differences. | DAGEngine | plan-diff, diff | `PlanDiff { added: [...], removed: [...], changed: [...] }` |
| PlanningHash | Deterministic SHA-256 hash of the full plan for replay auditing and tamper-evident verification. | PlanningPipeline | plan-hash, audit-hash | `PlanningHash { hash: "abc123..." }` |
| PlanningResult | Complete output of the planning phase: matched template, resolved parameters, validated TaskGraph, and deterministic hash. | PlanningPipeline | plan-result, generation-output | `PlanningResult { graph, hash, template_id }` |
| RAIIReservation | Rust idiom where budget reservations auto-release on Drop, ensuring no resource leaks. Used by BudgetTracking. | BudgetTracking | raii-guard, reservation | `let _res = budget.reserve(1000)?; // auto-released on scope exit` |
| ReadyQueue | O(1) deque of DAG nodes whose dependencies are all satisfied, waiting for an executor slot. | DAGEngine | runnable-queue | `graph.pop_ready_node()` |
| ResourceBudget | Tracked resource with type (tokens/calls/time), used count, soft warning limit, and hard enforcement limit. | Enforcement | budget, resource-cap | `ResourceBudget { resource: "tokens", used: 800, limit: 1000 }` |
| RetryDecision | Outcome of retry evaluation: Retry (with strategy + delay), Fallback (execute fallback node), Skip (mark skipped), or Abort (terminate execution). | ExecutionEngine | retry-outcome, decision | `RetryDecision::Retry { strategy, attempt, backoff_ms }` |
| RetryPolicy | Session-level or per-node retry configuration: max attempts, ordered retry strategies, backoff strategy, skip conditions. | ExecutionEngine | retry-config, retry-settings | `RetryPolicy { max_attempts: 4, retry_strategies: [SameOperation, ExpandContext] }` |
| RetryStrategy | Strategy for re-attempting a failed node: SameOperation, ExpandContext, SimplifyOperation, AlternateApproach, SkipAndContinue. | ExecutionEngine | retry-approach | `RetryStrategy::ExpandContext` |
| RiskGate | Decision point for tool execution: Low (auto-execute), Medium (user confirm), High (dry-run or block). | RiskGating | gate, gating-policy | `RiskGate::evaluate("rm -rf /") -> Block` |
| RiskLevel | Tool risk classification: Low (safe), Medium (needs confirmation), High (dangerous). | RiskGating | risk-tier, risk-category | `RiskLevel::Medium` |
| SealedGraph | A TaskGraph that has passed topological sort and cycle detection via `seal()`, frozen for execution. | DAGEngine | frozen-graph, validated-graph | `graph.seal() -> Ok(())` |
| SymbolGraph | Multi-language code index mapping symbol names to definitions, locations, and cross-references. O(1) name lookup. | RepoEngine | code-index, symbol-index | `SymbolGraph::lookup("authenticate") -> Vec<SymbolDefinition>` |
| TaskGraph | The core DAG structure with two-phase construction (add_unchecked -> seal). Contains TaskNodes, topological ordering, and execution state. | DAGEngine | graph, dag | `TaskGraph { nodes: [...], sealed: true }` |
| TaskNode | A single unit of work in the DAG: UUID, human-readable name, tool binding, dependency list, ExecutionPolicy, intent, optional ValidationRule. | DAGEngine | node, task | `TaskNode { id: uuid, name: "compile", tool: "cargo build" }` |
| TaskResult | Result of executing one node: success/failure flag, output string, duration, retry attempts, error details. | ExecutionEngine | node-result, task-output | `TaskResult { success: true, output: "Build succeeded" }` |
| Template | TOML-defined workflow template with parameter schema, node definitions, and dependency graph structure. | Templates | workflow-template, workflow-definition | Templates in `.rigorix/templates/*.toml` |
| TemplateDriven | Design principle: workflows are defined in reusable TOML templates, not dynamically generated by LLM at execution time. | Templates | — | Templates stored in `.rigorix/templates/` directory |
| UserIntent | Raw natural-language input from the user describing what they want to accomplish. | PlanningPipeline | prompt, request, command | `UserIntent("Add a POST endpoint to the users API")` |
| ValidationRule | Post-execution validation check: TypeCheck, TestPass, LintPass, or Custom command. | DAGEngine | post-check, validation | `ValidationRule::TestPass` |
| GeneratedTemplate | A TOML workflow template produced by the LLM generator. Contains the TOML string, suggested ID/name, and LLM usage statistics. | TemplateGeneration | llm-template, auto-template | `GeneratedTemplate { toml_content, suggested_id, suggested_name }` |
| TemplateGenerator | Trait for LLM-based template generation from user intent. Implemented by ClaudeTemplateGenerator using Anthropic's Messages API. | TemplateGeneration | generator, template-builder | `TemplateGenerator::generate(&self, intent, repo_context, budget)` |
| TemplatePersistenceService | Service that persists a generated template to `.rigorix/templates/<id>.toml` with atomic write-rename for crash safety. | TemplateGeneration | template-saver, generator-persistence | Saves generated templates to the templates directory |
| RepoContext | Snapshot of repository structure (file tree, deps, public API) used as context for LLM-based template generation. | TemplateGeneration | repo-snapshot, generation-context | `RepoContext { project_type, directory_tree, public_api }` |
| CliGenerateCommand | The `rigorix generate <intent>` CLI command that explicitly creates and persists a TOML template. Supports `--dry-run` (preview only) and `--stdout` (pipe output). | CLI | generate-command | `rigorix generate "add a POST endpoint" --dry-run` |

## Adding New Terms

1. Identify the term used in conversation and code
2. Add a row to the Glossary table
3. Define the term's **bounded context** (which module it lives in)
4. List any **aliases/synonyms** that agents might mistakenly use
5. Provide **code examples** showing correct usage
6. Run `.pi/scripts/validate-ubiquitous-language.sh` to detect drift

> **Rule of thumb:** If two agents use different names for the same concept, add an entry.
> The canonical term is the one used in the architecture module documents.
