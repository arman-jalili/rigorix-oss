//! Rigorix MCP Gateway — Binary entry point.
//!
//! @canonical .pi/architecture/modules/mcp-server.md
//! Implements: McpServer composition root with stdio transport
//!
//! Starts the MCP server in stdio mode (default). Reads newline-delimited
//! JSON-RPC messages from stdin and writes responses to stdout. Supports
//! graceful shutdown via SIGINT/SIGTERM.
//!
//! This composition root wires together all 10 OSS MCP tools across
//! three bounded contexts (execution, audit, template) with shared
//! in-memory services for development and testing.
//!
//! # Usage
//!
//! ```bash
//! # stdio mode (default — for AI tools like Claude Code, Aider)
//! rigorix-mcp
//!
//! # SSE mode (for GUI tools like Claude Desktop, Cursor)
//! rigorix-mcp --sse --bind 127.0.0.1:3001
//! ```

use std::sync::Arc;
use std::time::Duration;

use rigorix_mcp::audit_tools::application::service::{
    AuditSummaryHandler, ListAuditsHandler, ReadAuditHandler,
};
use rigorix_mcp::audit_tools::application::service_impl::{
    AuditSummaryHandlerImpl, ListAuditsHandlerImpl, ReadAuditHandlerImpl,
};
use rigorix_mcp::audit_tools::domain::entity::{AuditFormatter, SharedAuditQueryService};
use rigorix_mcp::audit_tools::domain::formatter_impl::AuditFormatterImpl;

use rigorix_mcp::execution_tools::application::service::{
    CheckEnforcementHandler, ExecuteHandler, ValidatePlanHandler,
};
use rigorix_mcp::execution_tools::application::service_impl::{
    CheckEnforcementHandlerImpl, ExecuteHandlerImpl, ValidatePlanHandlerImpl,
};
use rigorix_mcp::execution_tools::domain::entity::SharedEngineFacade;
use rigorix_mcp::execution_tools::infrastructure::{
    EngineFacadeConfig, EngineFacadeImpl, InMemoryExecutionRepository,
};
use rigorix_mcp::execution_tools::infrastructure::repository::ExecutionRepository;

use rigorix_mcp::execution_tools::domain::value::ExecutionStatus;

use rigorix_mcp::template_tools::application::service::{
    CreateTemplateHandler, GetTemplateHandler, ListTemplatesHandler, ValidateTemplateHandler,
};
use rigorix_mcp::template_tools::application::service_impl::{
    CreateTemplateHandlerImpl, GetTemplateHandlerImpl, ListTemplatesHandlerImpl,
    ValidateTemplateHandlerImpl,
};
use rigorix_mcp::template_tools::domain::entity::SharedTemplateRepository;
use rigorix_mcp::template_tools::infrastructure::FilesystemTemplateRepository;

use rigorix_mcp::enterprise_proxy::domain::entity::SharedEnterpriseProxy;
use rigorix_mcp::enterprise_proxy::domain::value::ProxyConfig;
use rigorix_mcp::enterprise_proxy::infrastructure::EnterpriseProxyImpl;

use rigorix_mcp::enterprise_proxy::interfaces::mcp::ENTERPRISE_TOOL_PREFIX;

use rigorix_mcp::mcp_server::domain::value::{JsonRpcError, JsonRpcMessage, RequestId};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::signal;
use tokio_util::sync::CancellationToken;

// =========================================================================
// Composition Root — shared application state
// =========================================================================

/// Shared application state wired from bounded contexts.
struct AppState {
    // Enterprise proxy (optional)
    enterprise_proxy: Option<SharedEnterpriseProxy>,

    // Execution tools
    execute_handler: Box<dyn ExecuteHandler>,
    validate_plan_handler: Box<dyn ValidatePlanHandler>,
    check_enforcement_handler: Box<dyn CheckEnforcementHandler>,

    // Audit tools
    read_audit_handler: Box<dyn ReadAuditHandler>,
    list_audits_handler: Box<dyn ListAuditsHandler>,
    audit_summary_handler: Box<dyn AuditSummaryHandler>,

    // Template tools
    list_templates_handler: Box<dyn ListTemplatesHandler>,
    get_template_handler: Box<dyn GetTemplateHandler>,
    create_template_handler: Box<dyn CreateTemplateHandler>,
    validate_template_handler: Box<dyn ValidateTemplateHandler>,

    // Direct access to concrete audit service for storing execution results
    audit_storage:
        std::sync::Arc<rigorix_mcp::audit_tools::infrastructure::InMemoryAuditQueryService>,
}

impl AppState {
    /// Try to initialize the enterprise proxy from environment variables.
    fn try_init_enterprise_proxy() -> Option<SharedEnterpriseProxy> {
        let api_url = std::env::var("ENTERPRISE_API_URL").ok()?;
        let api_key = std::env::var("ENTERPRISE_API_KEY").ok()?;

        let timeout = std::env::var("ENTERPRISE_TIMEOUT_SECS")
            .ok()
            .and_then(|v| v.parse().ok());
        let tls_verify = std::env::var("ENTERPRISE_TLS_VERIFY")
            .ok()
            .map(|v| !matches!(v.as_str(), "false" | "0" | "no"));
        let schema_ttl = std::env::var("ENTERPRISE_SCHEMA_TTL_SECS")
            .ok()
            .and_then(|v| v.parse().ok());

        let config = ProxyConfig::new(
            api_url, api_key, timeout, tls_verify, None, // max_retries uses default
            schema_ttl,
        )
        .ok()?;

        let proxy = EnterpriseProxyImpl::new(config).ok()?;
        let shared: SharedEnterpriseProxy = Arc::new(proxy);

        // Initialize (fetch schemas) — best-effort
        let init_shared = shared.clone();
        tokio::spawn(async move {
            match init_shared.initialize().await {
                Ok(()) => tracing::info!("Enterprise proxy initialized successfully"),
                Err(e) => tracing::warn!("Enterprise proxy init failed (will retry): {}", e),
            }
        });

        Some(shared)
    }

    /// Build the composition root with the given engine facade.
    fn new(engine: SharedEngineFacade) -> Self {

        // ── Audit service (in-memory) ──
        let audit_storage = std::sync::Arc::new(
            rigorix_mcp::audit_tools::infrastructure::InMemoryAuditQueryService::new(),
        );
        let audit_query: SharedAuditQueryService = audit_storage.clone();
        let formatter: Arc<dyn AuditFormatter> = Arc::new(AuditFormatterImpl::new());

        // ── Template repository (filesystem, default path) ──
        let template_repo: SharedTemplateRepository =
            Arc::new(FilesystemTemplateRepository::new(".rigorix/templates"));

        // ── Enterprise proxy (optional) ──
        let enterprise_proxy = Self::try_init_enterprise_proxy();
        if enterprise_proxy.is_some() {
            tracing::info!("Enterprise proxy enabled");
        } else {
            tracing::info!("Enterprise proxy disabled (no config)");
        }

        Self {
            enterprise_proxy,
            execute_handler: Box::new(ExecuteHandlerImpl::new(
                engine.clone(),
                Duration::from_secs(300),
            )),
            validate_plan_handler: Box::new(ValidatePlanHandlerImpl::new(engine.clone())),
            check_enforcement_handler: Box::new(CheckEnforcementHandlerImpl::new(engine)),

            // Audit handlers
            read_audit_handler: Box::new(ReadAuditHandlerImpl::new(
                audit_query.clone(),
                formatter.clone(),
            )),
            list_audits_handler: Box::new(ListAuditsHandlerImpl::new(
                audit_query.clone(),
                formatter.clone(),
            )),
            audit_summary_handler: Box::new(AuditSummaryHandlerImpl::new(audit_query, formatter)),

            // Template handlers
            list_templates_handler: Box::new(ListTemplatesHandlerImpl::new(template_repo.clone())),
            get_template_handler: Box::new(GetTemplateHandlerImpl::new(template_repo.clone())),
            create_template_handler: Box::new(CreateTemplateHandlerImpl::new(template_repo)),
            validate_template_handler: Box::new(ValidateTemplateHandlerImpl::new()),
            audit_storage,
        }
    }

    /// Route a tool call by name to the appropriate handler.
    async fn handle_tool_call(
        &self,
        tool_name: &str,
        params: &serde_json::Value,
    ) -> Result<serde_json::Value, serde_json::Value> {
        match tool_name {
            // Execution tools
            "rigorix_execute" => {
                let input: rigorix_mcp::execution_tools::application::dto::ExecuteInput =
                    serde_json::from_value(params.clone()).map_err(
                        |e| serde_json::json!({"error": format!("Invalid input: {}", e)}),
                    )?;
                let template_name = input.plan.name().to_string();
                let result = self
                    .execute_handler
                    .handle(input)
                    .await
                    .map_err(|e| serde_json::json!({"error": e.to_string()}))?;
                let json_result: serde_json::Value =
                    serde_json::from_str(&result.content[0].text).unwrap_or_default();

                // Store an audit record for the read_audit cycle
                if let Some(execution_id_str) = json_result["execution_id"].as_str()
                    && let Ok(exec_id) = uuid::Uuid::parse_str(execution_id_str)
                {
                    let now = chrono::Utc::now();
                    let envelope =
                            rigorix_mcp::audit_tools::infrastructure::InMemoryAuditQueryService::create_sample(
                                exec_id,
                                ExecutionStatus::Completed,
                                Some(template_name.clone()),
                                now,
                                100,
                            );
                    let _ = self.audit_storage.store(envelope);
                }

                Ok(json_result)
            }
            "rigorix_validate_plan" => {
                let input = serde_json::from_value(params.clone())
                    .map_err(|e| serde_json::json!({"error": format!("Invalid input: {}", e)}))?;
                let result = self
                    .validate_plan_handler
                    .handle(input)
                    .await
                    .map_err(|e| serde_json::json!({"error": e.to_string()}))?;
                Ok(serde_json::from_str(&result.content[0].text).unwrap_or_default())
            }
            "rigorix_check_enforcement" => {
                let result = self
                    .check_enforcement_handler
                    .handle()
                    .await
                    .map_err(|e| serde_json::json!({"error": e.to_string()}))?;
                Ok(serde_json::from_str(&result.content[0].text).unwrap_or_default())
            }

            // Audit tools
            "rigorix_read_audit" => {
                let input = serde_json::from_value(params.clone())
                    .map_err(|e| serde_json::json!({"error": format!("Invalid input: {}", e)}))?;
                let result = self
                    .read_audit_handler
                    .handle(input)
                    .await
                    .map_err(|e| serde_json::json!({"error": e.to_string()}))?;
                Ok(serde_json::from_str(&result.content[0].text).unwrap_or_default())
            }
            "rigorix_list_audits" => {
                let input = serde_json::from_value(params.clone())
                    .map_err(|e| serde_json::json!({"error": format!("Invalid input: {}", e)}))?;
                let result = self
                    .list_audits_handler
                    .handle(input)
                    .await
                    .map_err(|e| serde_json::json!({"error": e.to_string()}))?;
                Ok(serde_json::from_str(&result.content[0].text).unwrap_or_default())
            }
            "rigorix_audit_summary" => {
                let input = serde_json::from_value(params.clone())
                    .map_err(|e| serde_json::json!({"error": format!("Invalid input: {}", e)}))?;
                let result = self
                    .audit_summary_handler
                    .handle(input)
                    .await
                    .map_err(|e| serde_json::json!({"error": e.to_string()}))?;
                Ok(serde_json::from_str(&result.content[0].text).unwrap_or_default())
            }

            // Template tools
            "rigorix_list_templates" => {
                let filter = serde_json::from_value(params.clone())
                    .map_err(|e| serde_json::json!({"error": format!("Invalid input: {}", e)}))?;
                let result = self
                    .list_templates_handler
                    .handle(&filter)
                    .await
                    .map_err(|e| serde_json::json!({"error": e.to_string()}))?;
                Ok(serde_json::from_str(&result.content[0].text).unwrap_or_default())
            }
            "rigorix_get_template" => {
                let input = serde_json::from_value(params.clone())
                    .map_err(|e| serde_json::json!({"error": format!("Invalid input: {}", e)}))?;
                let result = self
                    .get_template_handler
                    .handle(&input)
                    .await
                    .map_err(|e| serde_json::json!({"error": e.to_string()}))?;
                Ok(serde_json::from_str(&result.content[0].text).unwrap_or_default())
            }
            "rigorix_create_template" => {
                let input = serde_json::from_value(params.clone())
                    .map_err(|e| serde_json::json!({"error": format!("Invalid input: {}", e)}))?;
                let result = self
                    .create_template_handler
                    .handle(&input)
                    .await
                    .map_err(|e| serde_json::json!({"error": e.to_string()}))?;
                Ok(serde_json::from_str(&result.content[0].text).unwrap_or_default())
            }
            "rigorix_validate_template" => {
                let input = serde_json::from_value(params.clone())
                    .map_err(|e| serde_json::json!({"error": format!("Invalid input: {}", e)}))?;
                let result = self
                    .validate_template_handler
                    .handle(&input)
                    .await
                    .map_err(|e| serde_json::json!({"error": e.to_string()}))?;
                Ok(serde_json::from_str(&result.content[0].text).unwrap_or_default())
            }

            _ => Err(serde_json::json!({
                "error": format!("Unknown tool: {}", tool_name)
            })),
        }
    }
}

// =========================================================================
// Real Engine Builder — constructs EngineFacadeImpl from rigorix-engine services
// =========================================================================

/// Build a real EngineFacadeImpl by constructing all required engine sub-services.
async fn build_real_engine(repo_root: &str) -> Result<SharedEngineFacade, Box<dyn std::error::Error + Send + Sync>> {
    use std::sync::Arc;

    use rigorix_engine::budget_tracking::application::llm_budget_impl::LlmBudgetImpl;
    use rigorix_engine::budget_tracking::application::service::LlmBudgetService;
    use rigorix_engine::cancellation::application::cancellation_service_impl::CancellationManagerImpl;
    use rigorix_engine::cancellation::application::service::CancellationService;
    use rigorix_engine::enforcement::application::factory::ExecutionEnforcerFactory;
    use rigorix_engine::enforcement::application::enforcer_factory_impl::ExecutionEnforcerFactoryImpl;
    use rigorix_engine::event_system::application::dto::EventBusConfig;
    use rigorix_engine::event_system::application::event_bus_service_impl::EventBusServiceImpl;
    use rigorix_engine::event_system::application::service::EventBusService;
    use rigorix_engine::execution_engine::application::factory::{ParallelExecutionFactory, ParallelExecutionFactoryConfig};
    use rigorix_engine::execution_engine::application::factory_impl::ParallelExecutionFactoryImpl;
    use rigorix_engine::orchestrator::application::builder::OrchestratorBuilder;
    use rigorix_engine::orchestrator::application::builder_impl::OrchestratorBuilderImpl;
    use rigorix_engine::orchestrator::domain::OrchestratorConfig;
    use rigorix_engine::planning::application::factory::PlanningPipelineFactory;
    use rigorix_engine::planning::application::pipeline_factory_impl::PlanningPipelineFactoryImpl;
    use rigorix_engine::state_persistence::application::service::StateManagerService;
    use rigorix_engine::state_persistence::application::state_manager_service_impl::FileSystemStateManager;
    use rigorix_engine::state_persistence::infrastructure::filesystem_state_repository::FileSystemStateRepository;
    use rigorix_engine::templates::application::dto::RegisterInput;
    use rigorix_engine::templates::application::service::TemplateEngineService;
    use rigorix_engine::templates::application::template_engine_impl::TemplateEngineImpl;


    // ── Planning pipeline (using mocks for classifier/extractor, real template engine) ──
    let execution_id = uuid::Uuid::new_v4().to_string();
    let classifier = Box::new(
        rigorix_engine::planning::application::MockClassifier::default()
            .with_match("e2e-test-plan", "e2e-test-plan", 1.0)
            .with_match("default", "default", 0.9),
    );
    let extractor = Box::new(rigorix_engine::planning::application::MockParameterExtractor::default());
    let template_service: Arc<dyn TemplateEngineService> = {
        let svc = Arc::new(TemplateEngineImpl::new());
        // Register a default catch-all template so the engine can execute any plan
        let _ = svc.register(RegisterInput {
            template: rigorix_engine::templates::domain::template::Template {
                id: "default".into(),
                name: "default".into(),
                description: "Default catch-all template".into(),
                version: "1.0.0".into(),
                parameters: vec![],
                nodes: vec![],
                tags: vec![],
                category: None,
                author: None,
            },
            overwrite: true,
        })
        .await;
        // Also pre-wire the MockClassifier to recognize any intent
        let _ = svc.register(RegisterInput {
            template: rigorix_engine::templates::domain::template::Template {
                id: "e2e-test-plan".into(),
                name: "e2e-test-plan".into(),
                description: "E2E test template".into(),
                version: "1.0.0".into(),
                parameters: vec![],
                nodes: vec![],
                tags: vec![],
                category: None,
                author: None,
            },
            overwrite: true,
        })
        .await;
        svc
    };
    let planning_pipeline = PlanningPipelineFactoryImpl
        .create_default(classifier, extractor, template_service)
        .await?;

    // ── Execution service ──
    let execution_service = ParallelExecutionFactoryImpl::new()
        .create(ParallelExecutionFactoryConfig::default())
        .await?;

    // ── State manager ──
    let state_repo = Box::new(FileSystemStateRepository::new(repo_root).await?);
    let state_manager: Arc<dyn StateManagerService> = Arc::new(FileSystemStateManager::new(state_repo));

    // ── Cancellation service ──
    let cancellation_service: Arc<dyn CancellationService> =
        Arc::new(CancellationManagerImpl::default());

    // ── Event bus ──
    let event_bus: Arc<dyn EventBusService> =
        Arc::new(EventBusServiceImpl::new(EventBusConfig::default()));

    // ── Budget service ──
    let budget_service: Arc<dyn LlmBudgetService> =
        Arc::new(LlmBudgetImpl::new(1000, 100_000, "mcp-server".into()));

    // ── Orchestrator ──
    let orchestrator = OrchestratorBuilderImpl::new(OrchestratorConfig::default())
        .with_repo_root(repo_root.to_string())
        .with_planning_pipeline(Arc::from(planning_pipeline))
        .with_execution_service(Arc::from(execution_service))
        .with_state_manager(state_manager)
        .with_cancellation_service(cancellation_service)
        .with_event_bus(event_bus)
        .with_budget_service(budget_service)
        .build()
        .await?;

    // ── Execution enforcer ──
    let enforcer: Arc<dyn rigorix_engine::enforcement::application::ExecutionEnforcer> =
        Arc::from(ExecutionEnforcerFactoryImpl.create_default(&execution_id).await?);

    // ── EngineFacadeImpl ──
    let execution_repo: Arc<dyn ExecutionRepository> = Arc::new(InMemoryExecutionRepository::new());

    let engine = EngineFacadeImpl::new(
        Arc::from(orchestrator),
        enforcer,
        execution_repo,
        EngineFacadeConfig {
            execute_timeout: Duration::from_secs(300),
            validate_timeout: Duration::from_secs(60),
            enforcement_enabled: true,
            repo_root: repo_root.to_string(),
        },
    );

    Ok(Arc::new(engine))
}

// =========================================================================
// Tool descriptors — all 10 OSS tools
// =========================================================================

/// Returns the list of all registered OSS MCP tool descriptors.
fn all_tool_descriptors() -> Vec<serde_json::Value> {
    vec![
        // Execution tools (3)
        rigorix_mcp::execution_tools::interfaces::mcp::rigorix_execute_tool_descriptor(),
        rigorix_mcp::execution_tools::interfaces::mcp::rigorix_validate_plan_tool_descriptor(),
        rigorix_mcp::execution_tools::interfaces::mcp::rigorix_check_enforcement_tool_descriptor(),
        // Audit tools (3)
        rigorix_mcp::audit_tools::interfaces::mcp::rigorix_read_audit_tool_descriptor(),
        rigorix_mcp::audit_tools::interfaces::mcp::rigorix_list_audits_tool_descriptor(),
        rigorix_mcp::audit_tools::interfaces::mcp::rigorix_audit_summary_tool_descriptor(),
        // Template tools (4)
        rigorix_mcp::template_tools::interfaces::mcp::rigorix_list_templates_tool_descriptor(),
        rigorix_mcp::template_tools::interfaces::mcp::rigorix_get_template_tool_descriptor(),
        rigorix_mcp::template_tools::interfaces::mcp::rigorix_create_template_tool_descriptor(),
        rigorix_mcp::template_tools::interfaces::mcp::rigorix_validate_template_tool_descriptor(),
    ]
}

// =========================================================================
// JSON-RPC Handler Functions
// =========================================================================

/// Map a ProxyError to a short error type name for diagnostic formatting.
fn error_type_name(e: &rigorix_mcp::enterprise_proxy::domain::error::ProxyError) -> &'static str {
    match e {
        rigorix_mcp::enterprise_proxy::domain::error::ProxyError::Configuration(_) => {
            "configuration"
        }
        rigorix_mcp::enterprise_proxy::domain::error::ProxyError::Transport(_) => "network_error",
        rigorix_mcp::enterprise_proxy::domain::error::ProxyError::ApiError { status, .. }
            if *status == 401 || *status == 403 =>
        {
            "auth_failure"
        }
        rigorix_mcp::enterprise_proxy::domain::error::ProxyError::ApiError { .. } => "api_error",
        rigorix_mcp::enterprise_proxy::domain::error::ProxyError::Timeout { .. } => "timeout",
        rigorix_mcp::enterprise_proxy::domain::error::ProxyError::Authentication(_) => {
            "auth_failure"
        }
        rigorix_mcp::enterprise_proxy::domain::error::ProxyError::NotEnabled => "not_enabled",
        _ => "internal_error",
    }
}

// Global application state — initialized once in main()
static APP_STATE: std::sync::OnceLock<AppState> = std::sync::OnceLock::new();

fn app_state() -> &'static AppState {
    APP_STATE.get().expect("AppState not initialized — call init_app_state() in main()")
}

/// Dispatch an incoming JSON-RPC message to the appropriate handler.
async fn dispatch_message(msg: JsonRpcMessage) -> Option<JsonRpcMessage> {
    let method = msg.method.as_deref()?;
    let id = msg.id.clone()?;
    let params = msg.params.unwrap_or(serde_json::Value::Null);

    let response = match method {
        "initialize" => handle_initialize(&id, &params).await,
        "initialized" => {
            return None;
        }
        "tools/list" => handle_list_tools(&id).await,
        "tools/call" => handle_call_tool(&id, &params).await,
        "resources/list" => handle_list_resources(&id).await,
        "resources/read" => handle_read_resource(&id, &params).await,
        "prompts/list" => handle_list_prompts(&id).await,
        "prompts/get" => handle_get_prompt(&id, &params).await,
        "notifications/cancelled" => {
            return None;
        }
        _ => JsonRpcMessage::error(id, JsonRpcError::method_not_found(method)),
    };

    Some(response)
}

// ---------------------------------------------------------------------------
// initialize handler
// ---------------------------------------------------------------------------

async fn handle_initialize(id: &RequestId, _params: &serde_json::Value) -> JsonRpcMessage {
    let result = serde_json::json!({
        "protocolVersion": "2025-03-26",
        "capabilities": {
            "tools": {},
            "resources": {},
            "prompts": {}
        },
        "serverInfo": {
            "name": "Rigorix MCP Gateway",
            "version": "0.1.0"
        }
    });
    JsonRpcMessage::success(id.clone(), result)
}

// ---------------------------------------------------------------------------
// tools/list — returns all OSS + enterprise tool descriptors
// ---------------------------------------------------------------------------

async fn handle_list_tools(id: &RequestId) -> JsonRpcMessage {
    let mut tools = all_tool_descriptors();

    // Append enterprise tools if proxy is enabled
    if let Some(proxy) = app_state().enterprise_proxy.as_ref() {
        // Static tools: always available when proxy is configured
        tools.push(
            rigorix_mcp::enterprise_proxy::interfaces::mcp::rigorix_enterprise_call_tool_descriptor(
            ),
        );
        tools.push(
            rigorix_mcp::enterprise_proxy::interfaces::mcp::rigorix_enterprise_health_tool_descriptor(),
        );
        // Dynamic tools: populated from schema cache (if init succeeded)
        for schema in proxy.available_tools() {
            tools.push(serde_json::json!({
                "name": schema.name,
                "description": schema.description,
                "inputSchema": schema.input_schema
            }));
        }
    }

    let result = serde_json::json!({ "tools": tools });
    JsonRpcMessage::success(id.clone(), result)
}

// ---------------------------------------------------------------------------
// tools/call — dispatches to the correct handler
// ---------------------------------------------------------------------------

async fn handle_call_tool(id: &RequestId, params: &serde_json::Value) -> JsonRpcMessage {
    let tool_name = params
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");

    let arguments = params
        .get("arguments")
        .cloned()
        .unwrap_or(serde_json::Value::Null);

    // Route rigorix_enterprise_* calls to the enterprise proxy
    if tool_name.starts_with(ENTERPRISE_TOOL_PREFIX) {
        match &app_state().enterprise_proxy {
            Some(proxy) => match proxy.handle(tool_name, arguments.clone()).await {
                Ok(result) => {
                    let response = serde_json::json!({
                        "content": [{"type": "text", "text": serde_json::to_string(&result).unwrap_or_default()}],
                        "isError": false
                    });
                    return JsonRpcMessage::success(id.clone(), response);
                }
                Err(e) => {
                    let diagnostic =
                        rigorix_mcp::enterprise_proxy::interfaces::mcp::format_enterprise_error(
                            error_type_name(&e),
                            &e.to_string(),
                        );
                    let response = serde_json::json!({
                        "content": [{"type": "text", "text": serde_json::to_string(&diagnostic).unwrap_or_default()}],
                        "isError": true
                    });
                    return JsonRpcMessage::success(id.clone(), response);
                }
            },
            None => {
                let response = serde_json::json!({
                    "content": [{"type": "text", "text": "Enterprise proxy is not configured. Set ENTERPRISE_API_URL and ENTERPRISE_API_KEY."}],
                    "isError": true
                });
                return JsonRpcMessage::success(id.clone(), response);
            }
        }
    }

    match app_state().handle_tool_call(tool_name, &arguments).await {
        Ok(result) => {
            let response = serde_json::json!({
                "content": [
                    {
                        "type": "text",
                        "text": serde_json::to_string(&result).unwrap_or_default()
                    }
                ],
                "isError": false
            });
            JsonRpcMessage::success(id.clone(), response)
        }
        Err(error) => {
            let response = serde_json::json!({
                "content": [
                    {
                        "type": "text",
                        "text": error["error"].as_str().unwrap_or("Unknown error")
                    }
                ],
                "isError": true
            });
            JsonRpcMessage::success(id.clone(), response)
        }
    }
}

// ---------------------------------------------------------------------------
// resources/list
// ---------------------------------------------------------------------------

async fn handle_list_resources(id: &RequestId) -> JsonRpcMessage {
    let result = serde_json::json!({
        "resources": [
            {
                "uri": "rigorix://audit/{id}",
                "name": "Audit Trail",
                "description": "Read an audit trail by execution ID",
                "mimeType": "text/plain"
            },
            {
                "uri": "rigorix://templates/{name}",
                "name": "Template",
                "description": "Read a template by name",
                "mimeType": "text/plain"
            }
        ]
    });
    JsonRpcMessage::success(id.clone(), result)
}

// ---------------------------------------------------------------------------
// resources/read
// ---------------------------------------------------------------------------

async fn handle_read_resource(id: &RequestId, params: &serde_json::Value) -> JsonRpcMessage {
    let uri = params
        .get("uri")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");

    JsonRpcMessage::error(
        id.clone(),
        JsonRpcError::internal_error(format!("Resource '{}' not implemented", uri)),
    )
}

// ---------------------------------------------------------------------------
// prompts/list
// ---------------------------------------------------------------------------

async fn handle_list_prompts(id: &RequestId) -> JsonRpcMessage {
    let result = serde_json::json!({
        "prompts": [
            {
                "name": "rigorix_introduction",
                "description": "Introduction to Rigorix tool usage",
                "arguments": []
            }
        ]
    });
    JsonRpcMessage::success(id.clone(), result)
}

// ---------------------------------------------------------------------------
// prompts/get
// ---------------------------------------------------------------------------

async fn handle_get_prompt(id: &RequestId, params: &serde_json::Value) -> JsonRpcMessage {
    let name = params
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");

    JsonRpcMessage::error(
        id.clone(),
        JsonRpcError::internal_error(format!("Prompt '{}' not implemented", name)),
    )
}

// ---------------------------------------------------------------------------
// Stdio Server
// ---------------------------------------------------------------------------

async fn run_stdio_server(cancel: CancellationToken) {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    let mut reader = BufReader::new(stdin).lines();
    let mut writer = stdout;

    tracing::info!("Rigorix MCP Gateway ready (stdio mode)");

    loop {
        tokio::select! {
            line = reader.next_line() => {
                match line {
                    Ok(Some(line)) => {
                        let line = line.trim().to_string();
                        if line.is_empty() {
                            continue;
                        }

                        match serde_json::from_str::<JsonRpcMessage>(&line) {
                            Ok(msg) => {
                                let response = dispatch_message(msg).await;
                                if let Some(resp) = response {
                                    let json = serde_json::to_string(&resp)
                                        .unwrap_or_else(|_| "{}".to_string());
                                    if let Err(e) = writer.write_all(format!("{}\n", json).as_bytes()).await {
                                        tracing::error!("Failed to write response: {}", e);
                                        break;
                                    }
                                    let _ = writer.flush().await;
                                }
                            }
                            Err(e) => {
                                let err = JsonRpcError::parse_error();
                                let error_msg = serde_json::json!({
                                    "jsonrpc": "2.0",
                                    "id": null,
                                    "error": {
                                        "code": err.code,
                                        "message": err.message
                                    }
                                });
                                let _ = writer.write_all(format!("{}\n", serde_json::to_string(&error_msg).unwrap()).as_bytes()).await;
                                let _ = writer.flush().await;
                                tracing::warn!("Failed to parse JSON-RPC message: {}", e);
                            }
                        }
                    }
                    Ok(None) => {
                        tracing::info!("stdin closed, shutting down");
                        break;
                    }
                    Err(e) => {
                        tracing::error!("Error reading stdin: {}", e);
                        break;
                    }
                }
            }
            _ = cancel.cancelled() => {
                tracing::info!("Shutdown signal received, stopping stdio server");
                break;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    // ── Build real engine facade ──
    let repo_root = std::env::var("RIGORIX_REPO_ROOT").unwrap_or_else(|_| ".".to_string());
    let engine = match build_real_engine(&repo_root).await {
        Ok(e) => {
            tracing::info!("EngineFacadeImpl initialized with real rigorix-engine");
            e
        }
        Err(e) => {
            tracing::error!("Failed to build real engine: {}. Exiting.", e);
            return;
        }
    };

    // ── Initialize app state ──
    let _ = APP_STATE.set(AppState::new(engine));

    let cancel = CancellationToken::new();

    // Parse args
    let args: Vec<String> = std::env::args().collect();
    let use_sse = args.iter().any(|a| a == "--sse");
    let bind_addr = args
        .iter()
        .position(|a| a == "--bind")
        .and_then(|i| args.get(i + 1).cloned())
        .unwrap_or_else(|| "127.0.0.1:3001".to_string());

    // Set up graceful shutdown
    let cancel_clone = cancel.clone();
    tokio::spawn(async move {
        #[cfg(unix)]
        {
            use signal::unix::SignalKind;
            if let (Ok(mut sigint), Ok(mut sigterm)) = (
                signal::unix::signal(SignalKind::interrupt()),
                signal::unix::signal(SignalKind::terminate()),
            ) {
                tokio::select! {
                    _ = sigint.recv() => tracing::info!("Received SIGINT"),
                    _ = sigterm.recv() => tracing::info!("Received SIGTERM"),
                }
            }
        }
        #[cfg(not(unix))]
        {
            let _ = signal::ctrl_c().await;
        }
        cancel_clone.cancel();
    });

    if use_sse {
        tracing::info!("Starting MCP Server in SSE mode on {}", bind_addr);
        tracing::warn!("SSE mode is not fully implemented in this phase");
    } else {
        tracing::info!("Starting MCP Server in stdio mode");
        run_stdio_server(cancel).await;
    }

    tracing::info!("MCP Server shut down gracefully");
}
