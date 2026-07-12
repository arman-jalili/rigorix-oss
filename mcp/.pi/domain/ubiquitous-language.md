# Ubiquitous Language

> Canonical glossary for **rigorix-oss**.
> All code MUST use these terms. Aliases/synonyms listed below are **prohibited** in source identifiers.
> Drift is detected by `.pi/scripts/validate-ubiquitous-language.sh`.

## Glossary

### Engine Core Terms

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
| **UserIntent** | Raw natural language request with working_directory, context k/v pairs, and clarification_history | Core Architecture | user-request, intent, query | `UserIntent::simple("add auth", PathBuf::from("/repo"))` |
| **CompositeValidator** | Aggregation of multiple PlanValidator implementations for combined validation | Planning Pipeline | validator-chain, multi-validator | `CompositeValidator::new().validate(&plan)` |
| **RiskLevel** | Enumeration of execution risk: Low (auto-execute), Medium (user confirm), High (dry-run) | Risk Gating | risk-class, danger-level | `RiskLevel::Low`, `RiskLevel::High` |
| **RiskClassifier** | Component that maps tool name to RiskLevel | Risk Gating | risk-analyzer, tool-scorer | `RiskClassifier::classify("run_command") -> High` |
| **RiskConfig** | Configurable risk gating policies per tool | Risk Gating | gate-config, risk-rule | `RiskConfig { auto_confirm: true }` |
| **SymbolGraph** | In-memory multi-language graph of code symbols with O(1) lookups | Repo Engine | code-graph, symbol-index | `SymbolGraph::lookup("fn_name")` |
| **SymbolDefinition** | A code symbol definition with name, kind, location, signature, documentation, and source_files | Repo Engine | symbol-def, code-definition | `SymbolDefinition { name: "parse", kind: Function }` |
| **ExecutionEnforcer** | Runtime component tracking hard caps on retries, tool calls, dynamic nodes, and execution time | Enforcement | enforcer, limit-tracker, cap-enforcer | `ExecutionEnforcer::new(EnforcementConfig::default_mode())` |
| **EnforcementConfig** | Configuration of hard caps across 3 autonomy presets: Default, Advanced, Aggressive | Enforcement | enforcement-settings, autonomy-config | `EnforcementConfig::advanced_mode()` |
| **EnforcementPreset** | Enum selecting autonomy mode: Default, Advanced, Aggressive | Configuration | mode-preset, autonomy-level | `EnforcementPreset::Default` |
| **LlmBudget** | RAII-managed budget for LLM calls and tokens with auto-rollback on Drop | Budget Tracking | token-budget, cost-tracker | `LlmBudget { max_calls: 5, max_tokens: 10000 }` |
| **EventBus** | Central pub-sub event bus with synchronous in-memory persistence | Event System | event-channel, message-bus | `EventBus::publish(ExecutionEvent::NodeStarted { .. })` |
| **ExecutionEvent** | Tagged union of observable events: PlanningStarted, NodeStarted, ToolExecuted, ExecutionCompleted | Event System | domain-event, notification | `ExecutionEvent::NodeCompleted { node_id, duration_ms }` |
| **AuditEnvelope** | Typed envelope for execution audit records with HMAC integrity | Audit | audit-record, governance-envelope | `AuditEnvelope::new(execution_id, events)` |
| **CircuitBreaker** | Circuit breaker pattern applied to audit HTTP requests with failure threshold, timeout, and half-open probe | Audit | circuit-break, fault-tolerance | `CircuitBreaker::new(max_failures: 5, timeout_secs: 60)` |
| **Secret** | API key wrapper with redacted Debug/Display/Serialize | Configuration | api-key, credential | `Secret::new("sk-...").expose()` |
| **TemplateGenerator** | Trait for LLM-based template creation from natural language intent + repo context | Template Generation | template-creator, workflow-generator | `impl TemplateGenerator for ClaudeTemplateGenerator` |
| **CoreOrchestratorError** | Root error type wrapping all sub-errors | Error Handling | root-error | `CoreOrchestratorError::Enforcement(EnforcementError::...)` |
| **CancellationToken** | Tokio-util CancellationToken that signals cancellation to all concurrent tasks | Cancellation | cancel-token, abort-signal | `CancellationToken::new()` |
| **Graceful** | Cancellation mode that lets running tasks finish before shutdown | Cancellation | soft-cancel, gentle-shutdown | `ShutdownSignal::Graceful` |
| **Immediate** | Cancellation mode that aborts all in-flight work | Cancellation | hard-cancel, force-stop | `ShutdownSignal::Immediate` |
| **ToolRegistry** | Registry of all available Tool implementations by name | Tool System | tool-catalog, tool-store | `ToolRegistry::register("file_read", FileReadTool)` |
| **ToolResult** | Output from a tool execution: output text, exit_code, side_effects | Tool System | tool-output, execution-result | `ToolResult { output: "file written", side_effects: ["wrote:src/main.rs"] }` |
| **ToolInput** | JSON parameters passed to a Tool's execute method | Tool System | tool-params, tool-args | `ToolInput::new(serde_json::json!({ "path": "src/lib.rs" }))` |
| **BoundedAutonomy** | Design principle capping dynamic behavior via 3 EnforcementConfig presets | Core Architecture | autonomy-cap, bounded-control | `EnforcementConfig::default_mode() // 0 dynamic nodes` |
| **TopologicalSort** | Kahn's algorithm ordering of TaskNodes ensuring dependencies execute before dependents | DAG Engine | topo-sort, dependency-order | `TaskGraph::topological_sort()` |
| **ParallelExecutor** | DAG executor using tokio JoinSet for parallel task execution with concurrency control | Execution Engine | dag-executor, task-runner | `ParallelExecutor::new().with_registry(tool_registry)` |
| **ExecutionRecord** | Complete record of an execution: context, planning metadata, events, task results | State Persistence | run-record, execution-log | `ExecutionRecord { context, planning_meta, events, task_results }` |

### MCP Gateway Terms

| Term | Definition | Bounded Context | Aliases/Synonyms | Examples |
|------|-----------|----------------|-----------------|---------|
| **McpServer** | Core aggregate root that manages MCP transport, sessions, tool registry, and request routing | MCP Server | mcp-gateway, protocol-server | `McpServer::new(config).serve().await` |
| **McpTransport** | Abstraction over MCP communication channel: stdio (stdin/stdout) or SSE (HTTP Server-Sent Events) | MCP Server | transport-layer, connection-mode | `McpTransport::Stdio`, `McpTransport::Sse { port: 3100 }` |
| **Session** | An active MCP client connection with negotiated capabilities, client metadata, and lifecycle state | MCP Server | client-session, connection, mcp-connection | `Session { id: "sess-1", client_info: ClientInfo::Claude, capabilities }` |
| **ToolRegistry** | Aggregate root that holds all registered MCP tools with their JSON schemas and handler functions | MCP Server | tool-catalog, handler-registry | `ToolRegistry::register("rigorix_execute", schema, handler)` |
| **RequestRouter** | Domain service that routes incoming tool calls to the correct handler based on tool name prefix | MCP Server | dispatcher, router | `RequestRouter::route(tool_call) -> Result<ToolResult>` |
| **ResourceProvider** | Domain service that exposes `rigorix://` URIs for read-only access to engine data | MCP Server | uri-provider, data-resolver | `ResourceProvider::read("rigorix://audit/latest")` |
| **PromptProvider** | Domain service that provides pre-crafted prompt templates for AI assistants | MCP Server | prompt-templates, guide-prompts | `PromptProvider::get("rigorix_execution_review")` |
| **JsonRpcMessage** | Value object representing a JSON-RPC 2.0 message (request, response, notification, or error) | MCP Server | rpc-message, protocol-packet | `JsonRpcMessage::Request { method: "tools/call", id: 1, params: {..} }` |
| **ToolSchema** | Value object describing an MCP tool's name, description, input parameters (JSON Schema), and output format | MCP Server | tool-definition, tool-spec | `ToolSchema { name: "rigorix_execute", description: "...", input_schema: {..} }` |
| **ResourceSchema** | Value object describing a resource's URI pattern, name, description, and MIME type | MCP Server | resource-definition, uri-schema | `ResourceSchema { uri: "rigorix://audit/{id}", name: "Audit Record" }` |
| **ServerCapabilities** | Value object representing negotiated capabilities advertised during MCP session initialization | MCP Server | capabilities, server-info | `ServerCapabilities { protocol_versions: ["2025-03-26"], tools: [...], resources: [...], enterprise_enabled: false }` |
| **EngineFacade** | Aggregate root providing a thin async facade over rigorix-engine public APIs for all MCP tool handlers | Execution Tools | engine-bridge, engine-adapter, engine-proxy | `EngineFacade::new(orchestrator, audit, enforcement, config)` |
| **ExecuteHandler** | Domain service that handles `rigorix_execute` tool calls: validates input, executes via engine, returns results | Execution Tools | execute-tool, plan-runner | `ExecuteHandler::handle(ExecuteInput { plan, template_name })` |
| **ValidatePlanHandler** | Domain service that handles `rigorix_validate_plan` tool calls: checks plan against enforcement policies | Execution Tools | plan-validator, preflight-checker | `ValidatePlanHandler::handle(ValidateInput { plan })` |
| **CheckEnforcementHandler** | Domain service that handles `rigorix_check_enforcement` tool calls: queries engine for current budget and limits | Execution Tools | enforcement-checker, budget-checker | `CheckEnforcementHandler::handle()` |
| **PlanTemplate** | Value object shared between execution and template contexts: a structured plan with steps, constraints, and metadata | Execution Tools, Template Tools | execution-plan, workflow-template, step-plan | `PlanTemplate { name: "refactor-auth", steps: [...], constraints: {...} }` |
| **StepDefinition** | Value object representing a single step in a plan: tool name, parameters, approval requirement, description | Execution Tools | plan-step, task-step, action-step | `StepDefinition { name: "add_dep", tool: "execute_command", config: {..}, requires_approval: false }` |
| **ExecutionResult** | Value object returned by `rigorix_execute`: execution_id, status, per-step results, duration, tokens, audit_uri | Execution Tools | run-result, plan-output | `ExecutionResult { execution_id: "rx-abc123", status: "completed", steps: [...], audit_uri: "rigorix://audit/rx-abc123" }` |
| **ValidationResult** | Value object returned by `rigorix_validate_plan`: valid flag, warnings, errors, estimated cost | Execution Tools | plan-validation, preflight-result | `ValidationResult { valid: true, warnings: [], errors: [], estimated_tokens: Some(50000) }` |
| **EnforcementStatus** | Value object returned by `rigorix_check_enforcement`: active, preset, remaining budget, circuit breaker states | Execution Tools | enforcement-state, budget-status | `EnforcementStatus { active: true, preset: "default", budget: BudgetStatus { tool_calls_remaining: 45 } }` |
| **AuditQueryService** | Aggregate root providing read-only access to execution audit records from rigorix-engine | Audit Tools | audit-reader, audit-lookup | `AuditQueryService::read_audit("rx-abc123")` |
| **ReadAuditHandler** | Domain service that handles `rigorix_read_audit` tool calls: retrieves audit by execution ID | Audit Tools | audit-reader-tool | `ReadAuditHandler::handle(ReadAuditInput { execution_id: "rx-abc123", format: "text" })` |
| **ListAuditsHandler** | Domain service that handles `rigorix_list_audits` tool calls: lists recent executions with filters | Audit Tools | audit-lister, audit-history | `ListAuditsHandler::handle(ListAuditsInput { status: Some("failed"), since: "2026-07-01", limit: 20 })` |
| **AuditSummaryHandler** | Domain service that handles `rigorix_audit_summary` tool calls: generates aggregate statistics | Audit Tools | audit-aggregator, audit-stats | `AuditSummaryHandler::handle(AuditSummaryInput { since: "2026-07-01" })` |
| **AuditFormatter** | Domain service that formats audit data for MCP consumption: human-readable markdown or structured JSON | Audit Tools | audit-presenter, audit-renderer | `AuditFormatter::format_audit_text(&envelope)` |
| **AuditFilter** | Value object with criteria for listing audit records: status, time range (since/until), template name, limit | Audit Tools | audit-query, filter-criteria | `AuditFilter { status: None, since: Some(timestamp), limit: 20 }` |
| **AuditSummary** | Value object with aggregate audit statistics over a time range: total executions, success rate, top failures, top templates | Audit Tools | aggregate-report, stats-summary | `AuditSummary { total_executions: 42, success_rate: 0.83, top_failures: [...], top_templates: [...] }` |
| **TemplateRepository** | Aggregate root managing filesystem storage of plan templates as TOML files in `.rigorix/templates/` | Template Tools | template-store, template-vault | `TemplateRepository::new(".rigorix/templates/")` |
| **ListTemplatesHandler** | Domain service that handles `rigorix_list_templates` tool calls: discovers templates from filesystem | Template Tools | template-lister, template-catalog | `ListTemplatesHandler::handle(TemplateFilter { tags: Some(vec!["auth"]), limit: 50 })` |
| **GetTemplateHandler** | Domain service that handles `rigorix_get_template` tool calls: reads and returns a specific template | Template Tools | template-reader, template-fetcher | `GetTemplateHandler::handle(GetTemplateInput { name: "refactor-auth", format: "json" })` |
| **CreateTemplateHandler** | Domain service that handles `rigorix_create_template` tool calls: validates and saves a new template | Template Tools | template-saver, template-writer | `CreateTemplateHandler::handle(CreateTemplateInput { name: "refactor-auth", plan: {..}, overwrite: false })` |
| **ValidateTemplateHandler** | Domain service that handles `rigorix_validate_template` tool calls: validates template structure | Template Tools | template-validator, schema-checker | `ValidateTemplateHandler::handle(ValidateTemplateInput { plan: {..} })` |
| **TemplateConverter** | Domain service that converts between TOML (filesystem storage) and JSON (MCP transport) template formats | Template Tools | template-codec, format-converter | `TemplateConverter::to_toml(&template)`, `TemplateConverter::to_json(&template)` |
| **TemplateFilter** | Value object with criteria for listing templates: tags, search text, result limit | Template Tools | template-query, search-criteria | `TemplateFilter { tags: Some(vec!["refactoring"]), search: Some("auth"), limit: 50 }` |
| **EnterpriseProxy** | Aggregate root that forwards `rigorix_enterprise_*` MCP tool calls to the Rigorix Enterprise API via HTTP JSON-RPC | Enterprise Proxy | enterprise-bridge, enterprise-gateway | `EnterpriseProxy::new(ProxyConfig { api_url, api_key, .. })` |
| **ProxyClient** | Domain service providing the HTTP client for JSON-RPC communication with the enterprise API | Enterprise Proxy | http-client, api-client | `ProxyClient::call(JsonRpcRequest { method: "rigorix_enterprise_team_audit", params: {..} })` |
| **SchemaCache** | Domain service that caches enterprise tool schemas for capability negotiation during MCP initialization | Enterprise Proxy | tool-discovery, schema-registry | `SchemaCache::update(enterprise_metadata)`, `SchemaCache::tools()` |
| **EnterpriseMetadata** | Value object returned by enterprise API during initialization: API version, available tools, capabilities | Enterprise Proxy | api-metadata, server-info | `EnterpriseMetadata { version: "1.0.0", tools: [...], capabilities: { team_audit: true } }` |
| **ProxyConfig** | Value object with enterprise connection settings: api_url, api_key (Secret), timeout, tls_verify, max_retries | Enterprise Proxy | enterprise-config, backend-config | `ProxyConfig { api_url: "https://enterprise.rigorix.io", api_key: Secret("rlx_...") }` |

## Adding New Terms

1. Identify the term used in conversation and code
2. Add a row to the Glossary table
3. Define the term's **bounded context** (which module it lives in)
4. List any **aliases/synonyms** that agents might mistakenly use
5. Provide **code examples** showing correct usage
6. Run `.pi/scripts/validate-ubiquitous-language.sh` to detect drift

> **Rule of thumb:** If two agents use different names for the same concept, add an entry.
> The canonical term is the one used in the architecture module documents.
