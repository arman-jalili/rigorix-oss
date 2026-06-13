# Ubiquitous Language

> Canonical glossary for **rigorix-oss**.
> All code MUST use these terms. Aliases/synonyms listed below are **prohibited** in source identifiers.
> Drift is detected by `.pi/scripts/validate-ubiquitous-language.sh`.

## Glossary

| Term | Definition | Bounded Context | Aliases/Synonyms | Examples |
|------|-----------|----------------|-----------------|---------|
| **Template** | A TOML file defining a workflow structure: nodes, actions, dependencies, parameters, retry config, and validation rules | Template System | workflow-definition, blueprint | `templates/code-review.toml` |
| **TemplateEngine** | Runtime registry that loads, registers, and instantiates templates into TaskGraphs with parameter substitution | Template System | template-registry, template-runtime | `TemplateEngine::register(template)` |
| **TemplateParser** | Parses TOML template files into Template structs, validating schema and action types | Template System | template-reader, toml-parser | `TemplateParser::parse_file("templates/refactor.toml")` |
| **ParameterDef** | A template parameter definition with name, description, type (string/path/boolean/number), required flag, and optional default | Template System | param-def, template-param | `ParameterDef { name: "target_file", param_type: Path }` |
| **TaskGraph** | A compiled Directed Acyclic Graph of task nodes with dependency edges and topological ordering | DAG Engine | workflow-graph, plan-graph, execution-dag | `TaskGraph::compile(template)` |
| **TaskNode** | A single node in the TaskGraph with ID, name, tool binding, parameters, dependencies, and execution policy | DAG Engine | task, node, work-item | `TaskNode { id: Uuid, tool_name: "file_write", deps: [...] }` |
| **ExecutionPolicy** | Per-node configuration for retry behavior: max_retries, retry_on failure types, retry_strategy, fallback_node, validation_rule, backoff_ms | DAG Engine | node-policy, retry-config | `ExecutionPolicy { max_retries: 3, retry_on: [Transient], backoff_ms: 100 }` |
| **ValidationRule** | Post-execution validation to apply: LintPass, TestPass, TypeCheck, Custom(cmd) | DAG Engine | post-check, output-validation | `ValidationRule::TypeCheck` |
| **FailureType** | Classification of execution failures: Transient, TestFailure, BuildFailure, LspConflict, ResourceExhausted, SystemError, NonRetryable | Failure Classification | failure-class, error-type | `FailureType::Transient` |
| **RetryStrategy** | Strategy for retrying failed operations: SameOperation, ReExecute, PatchWithFeedback, Fallback, ExpandContext | Execution Engine | retry-strategy, recovery-strategy | `RetryStrategy::PatchWithFeedback { feedback }` |
| **Plan** | A validated, executable mapping from template structure to concrete task nodes with resolved parameters | Planning Pipeline | execution-plan, workflow-plan | `let plan = task_graph_to_core_plan(&graph, template_id)` |
| **PlanningResult** | Deterministic contract from planning: selected_template, parameters, confidence, planning_hash, llm_calls, tokens_used | Planning Pipeline | plan-result, planning-output | `PlanningResult { selected_template, confidence, planning_hash }` |
| **PlanningHash** | SHA-256 of `intent.raw_text + template_id + sorted parameters` for deterministic replay | Planning Pipeline | plan-hash, execution-digest | `PlanningResult::compute_hash(intent, template_id, &params)` |
| **CLARIFICATION_THRESHOLD** | Confidence threshold (0.7) below which classifier flags clarification or triggers template generation | Planning Pipeline | low-confidence-threshold, gen-trigger | `if classification.confidence < 0.7 { clarify }` |
| **UserIntent** | Raw natural language request with working_directory, context k/v pairs, and clarification_history | Core Architecture | user-request, intent, query | `UserIntent::simple("add auth", PathBuf::from("/repo"))` |
| **CompositeValidator** | Aggregation of multiple PlanValidator implementations for combined validation | Planning Pipeline | validator-chain, multi-validator | `CompositeValidator::new().validate(&plan)` |
| **RiskLevel** | Enumeration of execution risk: Low (auto-execute), Medium (user confirm), High (dry-run) | Risk Gating | risk-class, danger-level | `RiskLevel::Low | Medium | High` |
| **RiskClassifier** | Component that maps tool name to RiskLevel (run_command→High, file_write→Medium, file_read→Low) | Risk Gating | risk-analyzer, tool-scorer | `RiskClassifier::classify("run_command") → High` |
| **RiskConfig** | Configurable risk gating policies per tool | Risk Gating | gate-config, risk-rule | `RiskConfig { auto_confirm: true }` |
| **SymbolGraph** | In-memory multi-language graph of code symbols (Rust, Python, TypeScript) with O(1) lookups | Repo Engine | code-graph, symbol-index, definition-graph | `SymbolGraph::lookup("fn_name")` |
| **SymbolDefinition** | A code symbol definition with name, kind, location, signature, documentation, and source_files | Repo Engine | symbol-def, code-definition | `SymbolDefinition { name: "parse", kind: Function, location }` |
| **ExecutionEnforcer** | Runtime component tracking hard caps on retries, tool calls, dynamic nodes, and execution time | Enforcement | enforcer, limit-tracker, cap-enforcer | `ExecutionEnforcer::new(EnforcementConfig::default_mode())` |
| **EnforcementConfig** | Configuration of hard caps across 3 autonomy presets: Default, Advanced, Aggressive | Enforcement | enforcement-settings, autonomy-config | `EnforcementConfig::advanced_mode()` |
| **EnforcementPreset** | Enum selecting autonomy mode: Default, Advanced, Aggressive | Configuration | mode-preset, autonomy-level | `EnforcementPreset::Default` |
| **LlmBudget** | RAII-managed budget for LLM calls and tokens with auto-rollback on Drop | Budget Tracking | token-budget, cost-tracker, usage-quota | `LlmBudget { max_calls: 5, max_tokens: 10000 }` |
| **LlmBudgetReservation** | RAII guard that holds budget capacity; commits actual tokens or auto-rolls back on Drop | Budget Tracking | budget-reservation, token-hold | `budget.reserve(estimated_tokens)?` |
| **EventBus** | Central pub-sub event bus (tokio broadcast) with synchronous in-memory persistence | Event System | event-channel, message-bus, event-stream | `EventBus::publish(ExecutionEvent::NodeStarted { .. })` |
| **ExecutionEvent** | Tagged union of observable events: PlanningStarted, NodeStarted, ToolExecuted, ExecutionCompleted, etc. | Event System | domain-event, notification, signal | `ExecutionEvent::NodeCompleted { node_id, duration_ms, output }` |
| **PersistedEvent** | An execution event with a monotonic sequence number for ordered replay | Event System | sequenced-event, replay-event | `PersistedEvent { sequence: 42, event: ExecutionEvent::... }` |
| **ExecutionState** | Serializable snapshot of an execution: status, node_states (IndexMap), symbol_graph_hash | State Persistence | run-state, execution-snapshot | `ExecutionState { execution_id, status: Running, node_states }` |
| **NodeState** | Per-node execution state: node_id, status (Pending/InProgress/Completed/Failed/Skipped), output, error, retries, duration_ms | State Persistence | node-status, task-state | `NodeState { node_id, status: Completed, duration_ms: Some(150) }` |
| **StateManager** | Persistence manager using atomic write-rename (`{id}.json.tmp` → `{id}.json`) with fd-lock | State Persistence | state-persister, execution-store | `StateManager::new(state_dir).await` |
| **ExecutionGraph** | Graph structure for TUI visualization of past executions | State Persistence | exec-graph, run-graph, history-graph | `ExecutionGraph::from_task_graph(execution_id, &graph, ...)` |
| **CancellationToken** | Tokio-util CancellationToken that signals cancellation to all concurrent tasks | Cancellation | cancel-token, abort-signal | `CancellationToken::new()` |
| **CancellationManager** | Manages graceful and immediate shutdown with watch channel for signal subscribers | Cancellation | cancel-manager, shutdown-controller | `CancellationManager::new()` |
| **ShutdownSignal** | Enum: Graceful (let tasks finish) or Immediate (abort in-flight work) | Cancellation | shutdown-level, abort-mode | `ShutdownSignal::Graceful` |
| **Secret** | API key wrapper with redacted Debug/Display/Serialize; accessible only via `.expose()` | Configuration | api-key, credential, sensitive-value | `Secret::new("sk-...").expose()` |
| **TemplateGenerator** | Trait for LLM-based template creation from natural language intent + repo context | Template Generation | template-creator, workflow-generator | `impl TemplateGenerator for ClaudeTemplateGenerator` |
| **ClaudeTemplateGenerator** | TemplateGenerator using Anthropic Messages API | Template Generation | claude-generator, anthropic-generator | `ClaudeTemplateGenerator::new(api_key, model)` |
| **OpenaiTemplateGenerator** | TemplateGenerator using OpenAI-compatible API | Template Generation | openai-generator, compatible-generator | `OpenaiTemplateGenerator::new(api_key, model, base_url)` |
| **RepoContext** | Repository snapshot for generation: dir_tree, project_type, dependencies, public_api, key_files, symbols | Template Generation | repo-metadata, workspace-context, project-profile | `RepoContext::from_path(&dir, &engine, Some(&symbols))` |
| **GeneratedTemplate** | A template produced by the generator, validated against schema and symbol graph | Template Generation | auto-template, synthetic-template, produced-template | `let t: Template = generator.generate(&intent, &ctx, &budget).await?` |
| **GeneratorError** | Typed error for generation failures: BudgetExhausted, LlmError, InvalidTOML, ValidationFailed, EmptyResponse, DuplicateTemplate, ContextError, Cancelled, SymbolValidation | Template Generation | generation-failure, gen-error | `GeneratorError::BudgetExhausted` |
| **BoundedAutonomy** | Design principle capping dynamic behavior via 3 EnforcementConfig presets | Core Architecture | autonomy-cap, bounded-control | `EnforcementConfig::default_mode() // 0 dynamic nodes` |
| **TopologicalSort** | Kahn's algorithm ordering of TaskNodes ensuring dependencies execute before dependents | DAG Engine | topo-sort, dependency-order | `TaskGraph::topological_sort()` |
| **PlanDiff** | Structured comparison between two planning decisions for auditing | Event System | plan-comparison, audit-diff | `PlanDiff::between(old_plan, new_plan)` |
| **ParallelExecutor** | DAG executor using tokio JoinSet for parallel task execution with concurrency control | Execution Engine | dag-executor, task-runner | `ParallelExecutor::new().with_registry(tool_registry)` |
| **ExecutionRecord** | Complete record of an execution: context, planning metadata, events, task results | State Persistence | run-record, execution-log | `ExecutionRecord { context, planning_meta, events, task_results }` |
| **CoreOrchestratorError** | Root error type wrapping DagError, PlanningError, EnforcementError, LlmBudgetError, ExecutionError, ToolError, SymbolGraphError, ConfigurationError | Error Handling | root-error, orchestrator-error | `CoreOrchestratorError::Enforcement(EnforcementError::...)` |
| **AuditEnvelope** | Typed envelope for execution audit records with HMAC integrity | Audit | audit-record, governance-envelope | `AuditEnvelope::new(execution_id, events)` |
| **Dogfooding** | Testing Rigorix by using Rigorix to build Rigorix features (self-hosting proof) | Core Architecture | self-hosting, eat-your-own-dogfood | `rigorix generate "Create a template generator module..."` |
| **ParallelExecutor** | DAG executor using tokio JoinSet for parallel task execution with concurrency control | Execution Engine | dag-executor, task-runner | `ParallelExecutor::new().with_registry(Arc::clone(&tool_registry))` |
| **Graceful** | Cancellation mode that lets running tasks finish before shutdown | Cancellation | soft-cancel, gentle-shutdown | `ShutdownSignal::Graceful` |
| **Immediate** | Cancellation mode that aborts all in-flight work | Cancellation | hard-cancel, force-stop, abort | `ShutdownSignal::Immediate` |
| **ToolRegistry** | Registry of all available Tool implementations by name | Tool System | tool-catalog, tool-store | `ToolRegistry::register("file_read", FileReadTool)` |
| **ToolResult** | Output from a tool execution: output text, exit_code, side_effects | Tool System | tool-output, execution-result | `ToolResult { output: "file written", side_effects: ["wrote:src/main.rs"] }` |
| **ToolInput** | JSON parameters passed to a Tool's execute method | Tool System | tool-params, tool-args | `ToolInput::new(serde_json::json!({ "path": "src/lib.rs" }))` |
| **SqlxAuditBackend** | Audit backend using SQLx for persistent audit storage in PostgreSQL | Audit | audit-db, audit-store | `SqlxAuditBackend::new(pool)` |
| **CircuitBreaker** | Circuit breaker pattern applied to audit HTTP requests with failure threshold, timeout, and half-open probe | Audit | circuit-break, fault-tolerance | `CircuitBreaker::new(max_failures: 5, timeout_secs: 60)` |

## Adding New Terms

1. Identify the term used in conversation and code
2. Add a row to the Glossary table
3. Define the term's **bounded context** (which module it lives in)
4. List any **aliases/synonyms** that agents might mistakenly use
5. Provide **code examples** showing correct usage
6. Run `.pi/scripts/validate-ubiquitous-language.sh` to detect drift

> **Rule of thumb:** If two agents use different names for the same concept, add an entry.
> The canonical term is the one used in the architecture module documents.
