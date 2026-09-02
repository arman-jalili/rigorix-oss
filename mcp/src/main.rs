//! Rigorix MCP Gateway — Binary entry point.
//!
//! @canonical .pi/architecture/modules/mcp-server.md
//! Implements: McpServer composition root with stdio transport
//!
//! Starts the MCP server in stdio mode (default). Reads newline-delimited
//! JSON-RPC messages from stdin and writes responses to stdout. Supports
//! graceful shutdown via SIGINT/SIGTERM.
//!
//! This composition root wires together all 15 OSS MCP tools across
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
//! rigorix-mcp (stdio mode)
//! ```

use std::sync::Arc;
use std::time::Duration;

use rigorix_engine::permission::domain::mode::PermissionMode;

use rigorix_mcp::audit_tools::application::service::{
    AuditSummaryHandler, ListAuditsHandler, ReadAuditHandler,
};
use rigorix_mcp::audit_tools::application::service_impl::{
    AuditSummaryHandlerImpl, ListAuditsHandlerImpl, ReadAuditHandlerImpl,
};
use rigorix_mcp::audit_tools::domain::entity::{
    AuditFormatter, AuditQueryService, SharedAuditQueryService,
};
use rigorix_mcp::audit_tools::domain::formatter_impl::AuditFormatterImpl;

use rigorix_mcp::execution_tools::application::service::{
    CheckEnforcementHandler, ExecuteHandler, PlanHandler, ValidatePlanHandler,
};
use rigorix_mcp::execution_tools::application::service_impl::{
    CheckEnforcementHandlerImpl, ExecuteHandlerImpl, PlanHandlerImpl, ValidatePlanHandlerImpl,
};
use rigorix_mcp::execution_tools::domain::entity::SharedEngineFacade;
use rigorix_mcp::execution_tools::infrastructure::repository::ExecutionRepository;
use rigorix_mcp::execution_tools::infrastructure::{
    EngineFacadeConfig, EngineFacadeImpl, InMemoryExecutionRepository,
};

use rigorix_mcp::template_tools::application::service::{
    CreateTemplateHandler, GetTemplateHandler, ListTemplatesHandler, ValidateTemplateHandler,
};
use rigorix_mcp::template_tools::application::service_impl::{
    CreateTemplateHandlerImpl, GetTemplateHandlerImpl, ListTemplatesHandlerImpl,
    ValidateTemplateHandlerImpl,
};
use rigorix_mcp::template_tools::domain::entity::SharedTemplateRepository;
use rigorix_mcp::template_tools::infrastructure::FilesystemTemplateRepository;

use rigorix_engine::configuration::domain::config::Config;
use rigorix_mcp::enterprise_proxy::domain::entity::SharedEnterpriseProxy;
use rigorix_mcp::enterprise_proxy::domain::value::ProxyConfig;
use rigorix_mcp::enterprise_proxy::infrastructure::EnterpriseProxyImpl;

use rigorix_mcp::enterprise_proxy::interfaces::mcp::ENTERPRISE_TOOL_PREFIX;

use rigorix_mcp::mcp_server::application::service::McpToolExecutor;
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

    // Engine facade (direct access for approval/sign-off flows)
    engine: SharedEngineFacade,

    // Execution tools
    execute_handler: Box<dyn ExecuteHandler>,
    plan_handler: Box<dyn PlanHandler>,
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

    // HMAC key used to sign envelopes stored for the read_audit cycle.
    audit_hmac_key: Option<String>,

    // Template repository for resolving template_name → plan
    template_repo: SharedTemplateRepository,

    // Live MCP server service — wired with the same handlers so the
    // `mcp_server` library module dispatches to production logic.
    mcp_service: Arc<dyn rigorix_mcp::mcp_server::application::service::McpServerService>,
}

/// Adapter that routes `mcp_server` protocol calls to the production handlers
/// held by the global [`AppState`].
struct AppStateExecutor;

#[async_trait::async_trait]
impl McpToolExecutor for AppStateExecutor {
    async fn execute_tool(
        &self,
        name: &str,
        arguments: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        app_state()
            .handle_tool_call(name, &arguments)
            .await
            .map_err(|e| e.to_string())
    }

    async fn read_resource(&self, uri: &str) -> Result<String, String> {
        resolve_resource(uri).await
    }
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

    /// Build the composition root with the given engine facade and template repository.
    fn new(
        engine: SharedEngineFacade,
        template_repo: SharedTemplateRepository,
        audit_hmac_key: Option<String>,
    ) -> Self {
        // ── Audit service (in-memory) ──
        let audit_storage = std::sync::Arc::new(
            rigorix_mcp::audit_tools::infrastructure::InMemoryAuditQueryService::new(),
        );
        let audit_query: SharedAuditQueryService = audit_storage.clone();
        let formatter: Arc<dyn AuditFormatter> = Arc::new(AuditFormatterImpl::new());

        // ── Enterprise proxy (optional) ──
        let enterprise_proxy = Self::try_init_enterprise_proxy();
        if enterprise_proxy.is_some() {
            tracing::info!("Enterprise proxy enabled");
        } else {
            tracing::info!("Enterprise proxy disabled (no config)");
        }

        Self {
            enterprise_proxy,
            engine: engine.clone(),
            execute_handler: Box::new(ExecuteHandlerImpl::new(
                engine.clone(),
                Duration::from_secs(300),
            )),
            plan_handler: Box::new(PlanHandlerImpl::new(engine.clone())),
            validate_plan_handler: Box::new(ValidatePlanHandlerImpl::new(engine.clone())),
            check_enforcement_handler: Box::new(CheckEnforcementHandlerImpl::new(engine.clone())),

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
            create_template_handler: Box::new(CreateTemplateHandlerImpl::new(
                template_repo.clone(),
            )),
            validate_template_handler: Box::new(ValidateTemplateHandlerImpl::new()),
            audit_storage,
            audit_hmac_key,
            mcp_service: {
                // Wire the mcp_server library module to the same handlers used
                // by the stdio server, so its protocol surface is live.
                let tool_schemas = all_tool_descriptors()
                    .into_iter()
                    .filter_map(|d| {
                        let name = d["name"].as_str()?.to_string();
                        let description = d["description"].as_str().unwrap_or("").to_string();
                        let input_schema = d.get("inputSchema").cloned().unwrap_or_default();
                        Some(rigorix_mcp::mcp_server::domain::value::ToolSchema::new(
                            name,
                            description,
                            input_schema,
                        ))
                    })
                    .collect();
                let executor: Arc<dyn McpToolExecutor> = Arc::new(AppStateExecutor);
                let service = rigorix_mcp::mcp_server::application::service_impl::
                    McpServerServiceWithRepos::new(
                        Arc::new(rigorix_mcp::mcp_server::infrastructure::
                            InMemoryMcpServerRepository::new()),
                        Arc::new(rigorix_mcp::mcp_server::infrastructure::
                            InMemoryToolRegistryRepository::new()),
                        Arc::new(rigorix_mcp::mcp_server::infrastructure::
                            InMemorySessionRepository::new()),
                    )
                    .with_executor(executor, tool_schemas);
                Arc::new(service)
                    as Arc<dyn rigorix_mcp::mcp_server::application::service::McpServerService>
            },
            template_repo,
        }
    }

    /// Resolve a template name to an execution PlanTemplate via the template repository.
    async fn resolve_template_to_execution_plan(
        &self,
        template_name: &str,
    ) -> Result<rigorix_mcp::execution_tools::domain::value::PlanTemplate, serde_json::Value> {
        let template = self.template_repo.get(template_name).await.map_err(|e| {
            serde_json::json!({"error": format!(
                "Template '{}' not found: {}",
                template_name, e
            )})
        })?;
        let json = serde_json::to_value(&template).unwrap_or_default();
        serde_json::from_value(json).map_err(|e| {
            serde_json::json!({"error": format!(
                "Failed to convert template '{}': {}",
                template_name, e
            )})
        })
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
                let mut input: rigorix_mcp::execution_tools::application::dto::ExecuteInput =
                    serde_json::from_value(params.clone()).map_err(
                        |e| serde_json::json!({"error": format!("Invalid input: {}", e)}),
                    )?;

                // Resolve template → plan if template_name is provided
                let template_name = if let Some(ref name) = input.template_name {
                    if input.plan.is_none() {
                        let plan = self.resolve_template_to_execution_plan(name).await?;
                        input.plan = Some(plan);
                    }
                    Some(name.clone())
                } else {
                    None
                };

                // Require at least a plan or template_name
                if input.plan.is_none() {
                    return Err(serde_json::json!({
                        "error": "Either 'plan' or 'template_name' must be provided"
                    }));
                }

                let template_name_for_audit = template_name
                    .or_else(|| input.plan.as_ref().map(|p| p.name().to_string()))
                    .unwrap_or_else(|| "unknown".to_string());

                let result = self
                    .execute_handler
                    .handle(input)
                    .await
                    .map_err(|e| serde_json::json!({"error": e.to_string()}))?;
                let json_result: serde_json::Value =
                    serde_json::from_str(&result.content[0].text).unwrap_or_default();

                // Store an audit record for the read_audit cycle — REAL envelope
                // built from the actual run result (steps, status, duration),
                // signed with the configured HMAC key when present.
                if let Some(execution_id_str) = json_result["execution_id"].as_str()
                    && let Ok(exec_id) = uuid::Uuid::parse_str(execution_id_str)
                {
                    let envelope = build_envelope_from_run(
                        &json_result,
                        exec_id,
                        template_name_for_audit,
                        &self.audit_hmac_key,
                        None,
                    );
                    let _ = self.audit_storage.store(envelope);
                }

                Ok(json_result)
            }
            "rigorix_plan" => {
                let input: rigorix_mcp::execution_tools::application::dto::PlanInput =
                    serde_json::from_value(params.clone()).map_err(
                        |e| serde_json::json!({"error": format!("Invalid input: {}", e)}),
                    )?;

                // Load full template (with version, tags, timestamps)
                let template = self
                    .template_repo
                    .get(&input.template_name)
                    .await
                    .map_err(|e| {
                        serde_json::json!({"error": format!(
                            "Template '{}' not found: {}",
                            input.template_name, e
                        )})
                    })?;

                let result = self
                    .plan_handler
                    .handle(&template)
                    .await
                    .map_err(|e| serde_json::json!({"error": e.to_string()}))?;

                Ok(serde_json::from_str(&result.content[0].text).unwrap_or_default())
            }
            "rigorix_run" => {
                let temp_name = params["template_name"].as_str().unwrap_or_default();

                // Load template via repo (handles both [[steps]] and [[nodes]] formats)
                let plan = self.resolve_template_to_execution_plan(temp_name).await?;

                // Execute via EngineFacade::execute() which uses run_from_template()
                // with evaluate_score from the converted steps
                let exec_input = rigorix_mcp::execution_tools::application::dto::ExecuteInput {
                    plan: Some(plan),
                    template_name: Some(temp_name.to_string()),
                    execution_id: params["execution_id"]
                        .as_str()
                        .and_then(|s| uuid::Uuid::parse_str(s).ok()),
                    repository: params["repository"].as_str().map(|s| s.to_string()),
                    author: params["author"].as_str().map(|s| s.to_string()),
                };

                let result = self
                    .execute_handler
                    .handle(exec_input)
                    .await
                    .map_err(|e| serde_json::json!({"error": e.to_string()}))?;
                let json_result: serde_json::Value =
                    serde_json::from_str(&result.content[0].text).unwrap_or_default();

                // Store audit record — REAL envelope from the run result.
                if let Some(execution_id_str) = json_result["execution_id"].as_str()
                    && let Ok(exec_id) = uuid::Uuid::parse_str(execution_id_str)
                {
                    let template_name = params["template_name"]
                        .as_str()
                        .unwrap_or_default()
                        .to_string();
                    let envelope = build_envelope_from_run(
                        &json_result,
                        exec_id,
                        template_name,
                        &self.audit_hmac_key,
                        None,
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
            "rigorix_approve_execution" => {
                use rigorix_mcp::execution_tools::domain::value::ExecutionId;

                let execution_id = params["execution_id"]
                    .as_str()
                    .and_then(|s| uuid::Uuid::parse_str(s).ok())
                    .ok_or_else(
                        || serde_json::json!({"error": "Invalid or missing execution_id"}),
                    )?;
                let step_names: Vec<String> = params["step_names"]
                    .as_array()
                    .ok_or_else(|| serde_json::json!({"error": "Missing step_names array"}))?
                    .iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect();
                if step_names.is_empty() {
                    return Err(serde_json::json!({
                        "error": "step_names must contain at least one step name"
                    }));
                }

                let approval = self
                    .engine
                    .approve_execution(&ExecutionId::from_uuid(execution_id), step_names)
                    .await
                    .map_err(|e| serde_json::json!({"error": e.to_string()}))?;

                // After a resumed approval, refresh the audit envelope with the
                // FINAL run state (all steps, statuses) so rigorix_read_audit
                // shows the completed runbook — not the stale paused snapshot.
                let mut final_state = None;
                if approval.resumed()
                    && let Ok(state) = self
                        .engine
                        .execution_state(&ExecutionId::from_uuid(execution_id))
                        .await
                {
                    // Reuse the stored envelope's template name if present.
                    let stored_template = self
                        .audit_storage
                        .read_audit(&ExecutionId::from_uuid(execution_id))
                        .await
                        .ok()
                        .and_then(|e| e.template_name().map(|s| s.to_string()))
                        .unwrap_or_else(|| "unknown".to_string());
                    let steps: Vec<serde_json::Value> = state
                        .node_states
                        .values()
                        .map(|s| {
                            serde_json::json!({
                                "step_name": s.node_name,
                                "success": s.status == "completed",
                                "error": s.last_error,
                                "duration_ms": s.last_duration_ms.unwrap_or(0),
                            })
                        })
                        .collect();
                    let refreshed = serde_json::json!({
                        "execution_id": execution_id.to_string(),
                        "status": if state.is_complete && state.failed_count == 0 {
                            "Completed"
                        } else if state.failed_count > 0 {
                            "Failed"
                        } else {
                            "PendingApproval"
                        },
                        "duration_ms": state.total_duration_ms,
                        "steps": steps,
                    });
                    // Use the REAL run start time from the engine session so
                    // the envelope's Started/Completed reflect the actual run.
                    let run_started = state.started_at.unwrap_or_else(chrono::Utc::now);
                    let envelope = build_envelope_from_run(
                        &refreshed,
                        execution_id,
                        stored_template,
                        &self.audit_hmac_key,
                        Some(run_started),
                    );
                    let _ = self.audit_storage.store(envelope);
                    final_state = Some(state);
                }

                Ok(serde_json::json!({
                    "execution_id": approval.execution_id().to_string(),
                    "approved_steps": approval.approved_steps(),
                    "not_found": approval.not_found(),
                    "still_pending": approval.still_pending(),
                    "resumed": approval.resumed(),
                    "message": if approval.resumed() {
                        "Approved — execution resumed"
                    } else if approval.still_pending().is_empty() {
                        "Approved — execution paused"
                    } else {
                        "Approval recorded — more steps still pending"
                    },
                    "final_state": final_state.map(|s| serde_json::json!({
                        "is_complete": s.is_complete,
                        "completed_count": s.completed_count,
                        "failed_count": s.failed_count,
                        "total_nodes": s.total_nodes,
                    })),
                }))
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
                // The handler may return markdown (text format) or JSON — pass
                // the text through, only parsing when it is actually JSON.
                let text = &result.content[0].text;
                Ok(serde_json::from_str(text)
                    .unwrap_or_else(|_| serde_json::Value::String(text.clone())))
            }
            "rigorix_list_audits" => {
                let input = serde_json::from_value(params.clone())
                    .map_err(|e| serde_json::json!({"error": format!("Invalid input: {}", e)}))?;
                let result = self
                    .list_audits_handler
                    .handle(input)
                    .await
                    .map_err(|e| serde_json::json!({"error": e.to_string()}))?;
                let text = &result.content[0].text;
                Ok(serde_json::from_str(text)
                    .unwrap_or_else(|_| serde_json::Value::String(text.clone())))
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

            // Usage guide tool
            "rigorix_get_usage_guide" => {
                Ok(rigorix_mcp::usage_guide::interfaces::mcp::handle_get_usage_guide())
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

// ── Helpers ──────────────────────────────────────────────────────────────

/// Build a REAL audit envelope from an execution run's JSON result.
///
/// The run result (rigorix_execute / rigorix_run) contains the actual steps,
/// status, and duration. We convert those into ExecutionStep records and sign
/// with the configured HMAC key (when present) — replacing the old fabricated
/// `create_sample` ("sample-hmac") so rigorix_read_audit returns honest
/// evidence: steps that actually ran, with a verifiable signature.
fn build_envelope_from_run(
    json_result: &serde_json::Value,
    exec_id: uuid::Uuid,
    template_name: String,
    hmac_key: &Option<String>,
    started_at: Option<chrono::DateTime<chrono::Utc>>,
) -> rigorix_mcp::audit_tools::domain::value::AuditEnvelope {
    use rigorix_mcp::audit_tools::domain::value::ExecutionStep;
    use rigorix_mcp::execution_tools::domain::value::ExecutionStatus as McpStatus;

    let status = match json_result["status"].as_str().unwrap_or("") {
        "Completed" => McpStatus::Completed,
        "Failed" => McpStatus::Failed,
        "PartialFailure" | "PartialFailed" => McpStatus::PartialFailed,
        "Cancelled" => McpStatus::Cancelled,
        "PendingApproval" => McpStatus::PendingApproval,
        _ => McpStatus::Completed,
    };
    let duration_ms = json_result["duration_ms"].as_u64().unwrap_or(0);

    let steps: Vec<ExecutionStep> = json_result["steps"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .map(|s| {
                    ExecutionStep::new(
                        s["step_name"].as_str().unwrap_or("?").to_string(),
                        s["success"].as_bool().unwrap_or(false),
                        s["error"].as_str().map(|e| e.to_string()),
                        s["output"].clone(),
                        s["duration_ms"].as_u64().unwrap_or(0),
                    )
                })
                .collect()
        })
        .unwrap_or_default();

    match started_at {
        Some(start) => {
            rigorix_mcp::audit_tools::infrastructure::InMemoryAuditQueryService::build_from_run_at(
                exec_id,
                status,
                Some(template_name),
                duration_ms,
                steps,
                hmac_key.as_deref(),
                start,
            )
        }
        None => {
            rigorix_mcp::audit_tools::infrastructure::InMemoryAuditQueryService::build_from_run(
                exec_id,
                status,
                Some(template_name),
                duration_ms,
                steps,
                hmac_key.as_deref(),
            )
        }
    }
}

/// Load a deserializable config struct from a TOML file in the repo root.
fn load_toml_config<T: serde::de::DeserializeOwned + Default>(
    repo_root: &str,
    filename: &str,
) -> T {
    let path = std::path::PathBuf::from(repo_root).join(filename);
    match std::fs::read_to_string(&path) {
        Ok(content) => toml::from_str(&content).unwrap_or_else(|e| {
            tracing::warn!("Failed to parse {} ({}); using defaults", path.display(), e);
            T::default()
        }),
        Err(e) => {
            tracing::warn!("Cannot read {} ({}); using defaults", path.display(), e);
            T::default()
        }
    }
}

/// Resolve the effective permission mode for the MCP engine.
///
/// Resolution order: rigorix.toml `permission_mode` → `RIGORIX_PERMISSION_MODE`
/// env var → `workspace_write` (safe default). Accepts both
/// `dangerous_full_access` (action.yml spelling) and `danger_full_access`
/// (engine serde spelling).
fn resolve_mcp_permission_mode(configured: Option<&String>) -> PermissionMode {
    let raw = configured
        .cloned()
        .or_else(|| std::env::var("RIGORIX_PERMISSION_MODE").ok())
        .unwrap_or_else(|| "workspace_write".to_string());
    match raw.trim().to_lowercase().as_str() {
        "read_only" => PermissionMode::ReadOnly,
        "dangerous_full_access" | "danger_full_access" => PermissionMode::DangerousFullAccess,
        _ => PermissionMode::WorkspaceWrite,
    }
}

/// Load a `HookRunnerService` from `.rigorix/hooks.toml` (optional).
///
/// When the file exists and parses as a `HookConfig`, every tool execution
/// runs the configured PreToolUse/PostToolUse shell hooks. Returns `None`
/// when the file is absent — never fatal.
fn load_mcp_hook_runner(
    repo_root: &str,
) -> Option<Arc<dyn rigorix_engine::hooks::application::service::HookRunnerService>> {
    use rigorix_engine::hooks::application::factory::HookRunnerFactory;
    use rigorix_engine::hooks::application::runner_factory_impl::HookRunnerFactoryImpl;
    use rigorix_engine::hooks::domain::config::HookConfig;

    let path = std::path::PathBuf::from(repo_root).join(".rigorix/hooks.toml");
    let content = std::fs::read_to_string(&path).ok()?;
    let config: HookConfig = toml::from_str(&content).ok()?;
    match HookRunnerFactoryImpl.create(config) {
        Ok(runner) => {
            tracing::info!("Hooks enabled from {}", path.display());
            Some(Arc::from(runner))
        }
        Err(e) => {
            tracing::warn!("hooks config invalid: {e}");
            None
        }
    }
}

/// GAP-A-07: mock classifier fallback (also used by e2e tests).
fn mock_classifier() -> Box<dyn rigorix_engine::planning::domain::classification::Classifier> {
    use rigorix_engine::planning::application::MockClassifier;
    Box::new(
        MockClassifier::default()
            // Catch-all: empty string matches any intent input
            .with_match("", "default", 0.1)
            .with_match("e2e-test-plan", "e2e-test-plan", 1.0)
            .with_match("default", "default", 0.9),
    )
}

/// Build a real EngineFacadeImpl by constructing all required engine sub-services.
async fn build_real_engine(
    repo_root: &str,
) -> Result<SharedEngineFacade, Box<dyn std::error::Error + Send + Sync>> {
    use std::sync::Arc;

    use rigorix_engine::budget_tracking::application::llm_budget_impl::LlmBudgetImpl;
    use rigorix_engine::budget_tracking::application::service::LlmBudgetService;
    use rigorix_engine::cancellation::application::cancellation_service_impl::CancellationManagerImpl;
    use rigorix_engine::cancellation::application::service::CancellationService;
    use rigorix_engine::enforcement::application::enforcer_factory_impl::ExecutionEnforcerFactoryImpl;
    use rigorix_engine::enforcement::application::factory::ExecutionEnforcerFactory;
    use rigorix_engine::event_system::application::dto::EventBusConfig;
    use rigorix_engine::event_system::application::event_bus_service_impl::EventBusServiceImpl;
    use rigorix_engine::event_system::application::service::EventBusService;
    use rigorix_engine::execution_engine::application::factory::{
        ParallelExecutionFactory, ParallelExecutionFactoryConfig,
    };
    use rigorix_engine::execution_engine::application::factory_impl::ParallelExecutionFactoryImpl;
    use rigorix_engine::execution_engine::application::service::ParallelExecutionService;
    use rigorix_engine::orchestrator::application::builder::OrchestratorBuilder;
    use rigorix_engine::orchestrator::application::builder_impl::OrchestratorBuilderImpl;
    use rigorix_engine::orchestrator::domain::OrchestratorConfig;
    use rigorix_engine::permission::application::enforcer_factory_impl::PermissionEnforcerFactoryImpl;
    use rigorix_engine::permission::application::factory::PermissionEnforcerFactory;
    use rigorix_engine::planning::application::factory::PlanningPipelineFactory;
    use rigorix_engine::planning::application::pipeline_factory_impl::PlanningPipelineFactoryImpl;
    use rigorix_engine::state_persistence::application::service::StateManagerService;
    use rigorix_engine::state_persistence::application::state_manager_service_impl::FileSystemStateManager;
    use rigorix_engine::state_persistence::infrastructure::filesystem_state_repository::FileSystemStateRepository;
    use rigorix_engine::templates::application::dto::RegisterInput;
    use rigorix_engine::templates::application::service::TemplateEngineService;
    use rigorix_engine::templates::application::template_engine_impl::TemplateEngineImpl;

    // ── Planning pipeline ──
    // GAP-A-07: prefer REAL LLM classifiers/extractor. Mock mode is used only
    // when explicitly requested (RIGORIX_MOCK_PLANNING=1) or no API key is set.
    let mock_planning = std::env::var("RIGORIX_MOCK_PLANNING")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);

    let classifier: Box<dyn rigorix_engine::planning::domain::classification::Classifier> =
        if let Ok(key) = std::env::var("ANTHROPIC_API_KEY") {
            if mock_planning {
                tracing::warn!("RIGORIX_MOCK_PLANNING=1 overrides ANTHROPIC_API_KEY — mock mode");
                mock_classifier()
            } else {
                tracing::info!("using real Claude classifier for planning");
                Box::new(
                    rigorix_engine::planning::infrastructure::ClaudeClassifier::new(
                        key,
                        Some(
                            rigorix_engine::planning::infrastructure::ClaudeClassifierConfig {
                                api_url: "https://api.anthropic.com/v1/messages".to_string(),
                                model: std::env::var("RIGORIX_PLANNING_MODEL")
                                    .unwrap_or_else(|_| "claude-sonnet-4-20250514".to_string()),
                                max_tokens: 1024,
                                temperature: 0.2,
                                timeout_secs: 120,
                                requests_per_second: 10,
                            },
                        ),
                    ),
                )
            }
        } else if let Ok(key) = std::env::var("OPENAI_API_KEY") {
            if mock_planning {
                tracing::warn!("RIGORIX_MOCK_PLANNING=1 overrides OPENAI_API_KEY — mock mode");
                mock_classifier()
            } else {
                tracing::info!("using real OpenAI classifier for planning");
                Box::new(
                    rigorix_engine::planning::infrastructure::OpenaiClassifier::new(
                        key,
                        Some(
                            rigorix_engine::planning::infrastructure::OpenaiClassifierConfig {
                                api_url: "https://api.openai.com/v1/chat/completions".to_string(),
                                model: std::env::var("RIGORIX_PLANNING_MODEL")
                                    .unwrap_or_else(|_| "gpt-4o".to_string()),
                                max_tokens: 1024,
                                temperature: 0.2,
                                timeout_secs: 120,
                                requests_per_second: 10,
                            },
                        ),
                    ),
                )
            }
        } else {
            tracing::warn!(
                "no ANTHROPIC_API_KEY/OPENAI_API_KEY — falling back to mock planning (set RIGORIX_MOCK_PLANNING=1 to silence)"
            );
            mock_classifier()
        };

    let execution_id = uuid::Uuid::new_v4().to_string();
    let extractor: Box<dyn rigorix_engine::planning::domain::extractor::ParameterExtractor> =
        if mock_planning
            || (std::env::var("ANTHROPIC_API_KEY").is_err()
                && std::env::var("OPENAI_API_KEY").is_err())
        {
            Box::new(rigorix_engine::planning::application::MockParameterExtractor::default())
        } else {
            let key = std::env::var("ANTHROPIC_API_KEY")
                .or_else(|_| std::env::var("OPENAI_API_KEY"))
                .unwrap_or_default();
            tracing::info!("using real LLM parameter extractor");
            Box::new(
                rigorix_engine::planning::infrastructure::LlmParameterExtractor::new(
                    key,
                    Some(
                        rigorix_engine::planning::infrastructure::LlmExtractorConfig {
                            api_url: std::env::var("ANTHROPIC_API_KEY")
                                .map(|_| "https://api.anthropic.com/v1/messages".to_string())
                                .unwrap_or_else(|_| {
                                    "https://api.openai.com/v1/chat/completions".to_string()
                                }),
                            model: std::env::var("RIGORIX_PLANNING_MODEL")
                                .unwrap_or_else(|_| "claude-sonnet-4-20250514".to_string()),
                            max_tokens: 1024,
                            temperature: 0.2,
                            timeout_secs: 120,
                            provider: if std::env::var("ANTHROPIC_API_KEY").is_ok() {
                                rigorix_engine::planning::infrastructure::ExtractorProvider::Anthropic
                            } else {
                                rigorix_engine::planning::infrastructure::ExtractorProvider::OpenAI
                            },
                        },
                    ),
                ),
            )
        };
    let template_service: Arc<dyn TemplateEngineService> = {
        let svc = Arc::new(TemplateEngineImpl::new());
        // Register a default catch-all template so the engine can execute any plan
        let _ = svc
            .register(RegisterInput {
                template: rigorix_engine::templates::domain::template::Template {
                    id: "default".into(),
                    name: "default".into(),
                    description: "Default catch-all template".into(),
                    version: "1.0.0".into(),
                    parameters: vec![],
                    nodes: vec![rigorix_engine::templates::domain::template::TemplateNode {
                        id: "step-1".into(),
                        name: "default-step".into(),
                        depends_on: vec![],
                        action:
                            rigorix_engine::templates::domain::template::TemplateAction::FileRead {
                                path: format!("{}/README.md", repo_root),
                            },
                        description: Some("Default execution step".into()),
                        retry: Default::default(),
                        validate: vec![],
                        requires_approval: false,
                        intent: None,
                    }],
                    tags: vec![],
                    category: None,
                    author: None,
                },
                overwrite: true,
            })
            .await;
        // Also pre-wire the MockClassifier to recognize any intent
        let _ = svc
            .register(RegisterInput {
                template: rigorix_engine::templates::domain::template::Template {
                    id: "e2e-test-plan".into(),
                    name: "e2e-test-plan".into(),
                    description: "E2E test template".into(),
                    version: "1.0.0".into(),
                    parameters: vec![],
                    nodes: vec![rigorix_engine::templates::domain::template::TemplateNode {
                        id: "step-1".into(),
                        name: "e2e-step".into(),
                        depends_on: vec![],
                        action:
                            rigorix_engine::templates::domain::template::TemplateAction::FileRead {
                                path: format!("{}/README.md", repo_root),
                            },
                        description: Some("E2E test step".into()),
                        retry: Default::default(),
                        validate: vec![],
                        requires_approval: false,
                        intent: None,
                    }],
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

    // ── Engine config (rigorix.toml: audit HMAC key, permission mode) ──
    use rigorix_engine::configuration::domain::config::Config;
    let engine_config = load_toml_config::<Config>(repo_root, "rigorix.toml");

    // ── Execution service ──
    // Permission mode: rigorix.toml → RIGORIX_PERMISSION_MODE env → workspace_write.
    let permission_mode = resolve_mcp_permission_mode(engine_config.permission_mode.as_ref());
    let permission_enforcer: Option<
        Arc<dyn rigorix_engine::permission::application::enforcer::PermissionEnforcer>,
    > = match PermissionEnforcerFactoryImpl
        .create_with_mode(permission_mode)
        .await
    {
        Ok(enforcer) => Some(Arc::from(enforcer)),
        Err(e) => {
            tracing::warn!("permission enforcer unavailable ({e}); continuing without mode gating");
            None
        }
    };
    // Hooks: optional `.rigorix/hooks.toml` → PreToolUse/PostToolUse interception.
    let hook_runner = load_mcp_hook_runner(repo_root);
    let execution_service: Arc<dyn ParallelExecutionService> = Arc::from(
        ParallelExecutionFactoryImpl::new()
            .create(ParallelExecutionFactoryConfig {
                permission_enforcer,
                hook_runner,
                ..ParallelExecutionFactoryConfig::default()
            })
            .await?,
    );

    // ── State manager ──
    let state_dir = std::path::PathBuf::from(repo_root)
        .join(".rigorix")
        .join("state");
    let state_repo = Box::new(FileSystemStateRepository::new(state_dir).await?);
    let state_manager: Arc<dyn StateManagerService> =
        Arc::new(FileSystemStateManager::new(state_repo));

    // ── Cancellation service ──
    let cancellation_service: Arc<dyn CancellationService> =
        Arc::new(CancellationManagerImpl::default());

    // ── Event bus ──
    let event_bus: Arc<dyn EventBusService> =
        Arc::new(EventBusServiceImpl::new(EventBusConfig::default()));

    // ── Budget service (configurable via rigorix.toml; each runbook step
    //    consumes one call — a tight budget makes rigorix_run refuse) ──
    let budget_service: Arc<dyn LlmBudgetService> = Arc::new(LlmBudgetImpl::new(
        engine_config.budget_max_calls.unwrap_or(1000),
        engine_config.budget_max_tokens.unwrap_or(100_000),
        "mcp-server".into(),
    ));

    // ── Audit service (reads audit_backend_url / audit_backend_key from rigorix.toml) ──
    use rigorix_engine::audit::application::AuditService;
    use rigorix_engine::audit::application::audit_queue_impl::AuditQueueImpl;
    use rigorix_engine::audit::application::audit_sender_impl::AuditSenderImpl;
    use rigorix_engine::audit::application::audit_service_impl::AuditServiceImpl;
    use rigorix_engine::audit::application::envelope_factory_impl::AuditEnvelopeFactoryImpl;

    let audit_url = engine_config.audit_backend_url.clone();
    let audit_key = engine_config.audit_backend_key.clone();
    let audit_sender =
        Arc::new(AuditSenderImpl::new(None, audit_url.clone()).with_api_key(audit_key.clone()));
    // HMAC signing key: rigorix.toml `audit_hmac_key` or RIGORIX_HMAC_KEY env.
    let hmac_key = engine_config
        .audit_hmac_key
        .clone()
        .or_else(|| std::env::var("RIGORIX_HMAC_KEY").ok())
        .filter(|k| !k.is_empty());
    let audit_service: Arc<dyn AuditService> = Arc::new(AuditServiceImpl::new(
        Box::new(AuditEnvelopeFactoryImpl::new(hmac_key)),
        audit_sender,
        Box::new(AuditQueueImpl::default()),
        audit_url.is_some(),
    ));

    if audit_url.is_some() {
        tracing::info!("Audit backend configured via rigorix.toml");
    }

    // ── ScoredEvaluationService (optional, from .rigorix/scored_evaluation.toml) ──
    use rigorix_engine::scored_evaluation::application::ScoredEvaluationService;
    use rigorix_engine::scored_evaluation::application::ScoredEvaluationServiceImpl;
    use rigorix_engine::scored_evaluation::infrastructure::LocalEvaluationRepository;
    use rigorix_engine::scored_evaluation::infrastructure::backends::{
        HttpBackend, LocalBackend, McpBackend,
    };
    use std::collections::HashMap;

    let rigorix_dir = std::path::PathBuf::from(repo_root).join(".rigorix");
    let se_config_path = rigorix_dir.join("scored_evaluation.toml");
    let mut builder = OrchestratorBuilderImpl::new(OrchestratorConfig::default())
        .with_repo_root(repo_root.to_string())
        .with_planning_pipeline(Arc::from(planning_pipeline))
        .with_execution_service(Arc::clone(&execution_service))
        .with_state_manager(state_manager)
        .with_cancellation_service(cancellation_service)
        .with_event_bus(event_bus)
        .with_audit_service(audit_service)
        .with_budget_service(budget_service);

    if se_config_path.exists() {
        match std::fs::read_to_string(&se_config_path) {
            Ok(content) => {
                let parsed: Result<serde_json::Value, _> = toml::from_str(&content);
                if let Ok(val) = parsed {
                    let backends_conf = val
                        .get("scored_evaluation")
                        .and_then(|s| s.get("backends"))
                        .and_then(|b| b.as_object());
                    if let Some(bconf) = backends_conf {
                        let mut backends: HashMap<
                            String,
                            Box<dyn rigorix_engine::scored_evaluation::domain::ScoringBackend>,
                        > = HashMap::new();
                        for (name, conf) in bconf {
                            let backend_type =
                                conf.get("type").and_then(|t| t.as_str()).unwrap_or("");
                            let timeout = conf
                                .get("timeout_ms")
                                .and_then(|t| t.as_u64())
                                .unwrap_or(30_000);
                            match backend_type {
                                "local" => {
                                    if let Some(script) =
                                        conf.get("script_path").and_then(|s| s.as_str())
                                    {
                                        let full_path = rigorix_dir
                                            .parent()
                                            .map(|p| p.join(script))
                                            .unwrap_or_else(|| std::path::PathBuf::from(script));
                                        backends.insert(
                                            name.clone(),
                                            Box::new(LocalBackend::new(
                                                full_path.to_string_lossy().to_string(),
                                                timeout,
                                            )),
                                        );
                                    }
                                }
                                "http" => {
                                    if let Some(url) = conf.get("url").and_then(|u| u.as_str()) {
                                        backends.insert(
                                            name.clone(),
                                            Box::new(HttpBackend::new(
                                                url.to_string(),
                                                HashMap::new(),
                                                timeout,
                                            )),
                                        );
                                    }
                                }
                                "mcp" => {
                                    if let Some(url) = conf.get("url").and_then(|u| u.as_str()) {
                                        backends.insert(
                                            name.clone(),
                                            Box::new(McpBackend::new(url.to_string(), timeout)),
                                        );
                                    }
                                }
                                _ => {}
                            }
                        }
                        if !backends.is_empty() {
                            let eval_repo = Box::new(LocalEvaluationRepository::new(
                                rigorix_dir.join("evaluations"),
                            ));
                            let se_svc =
                                Arc::new(ScoredEvaluationServiceImpl::new(backends, eval_repo))
                                    as Arc<dyn ScoredEvaluationService>;
                            builder = builder.with_scored_evaluation_service(se_svc);
                            tracing::info!(
                                "Scored evaluation service wired from {}",
                                se_config_path.display()
                            );
                        }
                    }
                }
            }
            Err(e) => {
                tracing::warn!("Failed to read {}: {}", se_config_path.display(), e);
            }
        }
    }

    let orchestrator = builder.build().await?;

    // ── Execution enforcer ──
    // Enforcement config: local defaults, optionally merged from a remote
    // backend (rigorix.toml `enforcement_backend_url` / `enforcement_backend_key`).
    use rigorix_engine::backend::EnforcementConfigProvider;
    use rigorix_engine::enforcement::domain::config::EnforcementConfig;
    let local_enforcement = EnforcementConfig::default();
    let enforcement_config = match &engine_config.enforcement_backend_url {
        Some(url) => {
            let provider = rigorix_engine::backend::HttpEnforcementConfigProvider::new(
                url.clone(),
                engine_config.enforcement_backend_key.clone(),
                std::time::Duration::from_secs(10),
            );
            match provider.fetch_merged_config(&local_enforcement).await {
                Ok(Some(merged)) => {
                    tracing::info!("Remote enforcement config applied from {url}");
                    merged
                }
                Ok(None) => {
                    tracing::debug!(
                        "Remote enforcement backend returned no override; using local config"
                    );
                    local_enforcement
                }
                Err(e) => {
                    tracing::warn!(
                        "Failed to fetch remote enforcement config ({e}); using local defaults"
                    );
                    local_enforcement
                }
            }
        }
        None => local_enforcement,
    };
    let enforcer: Arc<dyn rigorix_engine::enforcement::application::ExecutionEnforcer> = Arc::from(
        ExecutionEnforcerFactoryImpl
            .create_from_config(&execution_id, enforcement_config)
            .await?,
    );

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

// Build the intent formatter — LLM-based when provider env vars are set,
// JSON fallback otherwise.
// =========================================================================
// Tool descriptors — all OSS tools
// =========================================================================

/// Returns the list of all registered OSS MCP tool descriptors.
fn all_tool_descriptors() -> Vec<serde_json::Value> {
    vec![
        // Execution tools (6)
        rigorix_mcp::execution_tools::interfaces::mcp::rigorix_execute_tool_descriptor(),
        rigorix_mcp::execution_tools::interfaces::mcp::rigorix_plan_tool_descriptor(),
        rigorix_mcp::execution_tools::interfaces::mcp::rigorix_run_tool_descriptor(),
        rigorix_mcp::execution_tools::interfaces::mcp::rigorix_validate_plan_tool_descriptor(),
        rigorix_mcp::execution_tools::interfaces::mcp::rigorix_check_enforcement_tool_descriptor(),
        rigorix_mcp::execution_tools::interfaces::mcp::rigorix_approve_execution_tool_descriptor(),
        // Audit tools (3)
        rigorix_mcp::audit_tools::interfaces::mcp::rigorix_read_audit_tool_descriptor(),
        rigorix_mcp::audit_tools::interfaces::mcp::rigorix_list_audits_tool_descriptor(),
        rigorix_mcp::audit_tools::interfaces::mcp::rigorix_audit_summary_tool_descriptor(),
        // Template tools (4)
        rigorix_mcp::template_tools::interfaces::mcp::rigorix_list_templates_tool_descriptor(),
        rigorix_mcp::template_tools::interfaces::mcp::rigorix_get_template_tool_descriptor(),
        rigorix_mcp::template_tools::interfaces::mcp::rigorix_create_template_tool_descriptor(),
        rigorix_mcp::template_tools::interfaces::mcp::rigorix_validate_template_tool_descriptor(),
        // Usage guide (1)
        rigorix_mcp::usage_guide::interfaces::mcp::rigorix_get_usage_guide_tool_descriptor(),
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
    APP_STATE
        .get()
        .expect("AppState not initialized — call init_app_state() in main()")
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

async fn handle_initialize(id: &RequestId, params: &serde_json::Value) -> JsonRpcMessage {
    // GAP-A-22: route initialize through the runtime McpServerService — this
    // creates the session and negotiates the protocol version (no fabricated
    // response).
    let protocol_version = params
        .get("protocolVersion")
        .and_then(|v| v.as_str())
        .unwrap_or("2025-03-26")
        .to_string();
    let client_info = {
        let ci = params.get("clientInfo").cloned().unwrap_or_default();
        rigorix_mcp::mcp_server::domain::value::ClientInfo {
            name: ci
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string(),
            version: ci
                .get("version")
                .and_then(|v| v.as_str())
                .map(str::to_string),
        }
    };
    let caps = params.get("capabilities").cloned().unwrap_or_default();
    let input = rigorix_mcp::mcp_server::application::dto::InitializeInput {
        protocol_version: protocol_version.clone(),
        client_info: client_info.clone(),
        capabilities: rigorix_mcp::mcp_server::domain::value::ClientCapabilities {
            protocol_version: protocol_version.clone(),
            client_name: Some(client_info.name.clone()),
            client_version: client_info.version.clone(),
            supports_progress: caps
                .get("experimental")
                .and_then(|e| e.get("progress"))
                .and_then(|p| p.as_bool())
                .unwrap_or(false),
        },
    };

    match app_state().mcp_service.initialize(input).await {
        Ok((output, _events)) => {
            let result = serde_json::json!({
                "protocolVersion": output.protocol_version,
                "capabilities": {
                    "tools": {},
                    "resources": {},
                    "prompts": {}
                },
                "serverInfo": {
                    "name": output.server_info,
                    "version": "0.1.0"
                }
            });
            JsonRpcMessage::success(id.clone(), result)
        }
        Err(err) => JsonRpcMessage::error(
            id.clone(),
            JsonRpcError::internal_error(format!("initialize failed: {err}")),
        ),
    }
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
        .unwrap_or("unknown")
        .to_string();

    // Route through the wired mcp_server service so the library module is
    // the live backend for the advertised resources.
    let input = rigorix_mcp::mcp_server::application::dto::ReadResourceInput { uri: uri.clone() };
    match app_state().mcp_service.read_resource(input).await {
        Ok(output) => {
            let result = serde_json::json!({
                "contents": [
                    {
                        "uri": output.uri,
                        "mimeType": output.mime_type,
                        "text": output.text
                    }
                ]
            });
            JsonRpcMessage::success(id.clone(), result)
        }
        Err(err) => JsonRpcMessage::error(
            id.clone(),
            JsonRpcError::invalid_params(format!("Resource read failed: {err}")),
        ),
    }
}

/// Resolve an advertised `rigorix://` resource to its text content.
///
/// Backs the resources advertised in `resources/list`:
/// - `rigorix://audit/{id}`        → formatted audit trail for an execution
/// - `rigorix://templates/{name}`  → template definition (JSON)
async fn resolve_resource(uri: &str) -> Result<String, String> {
    let (scheme, rest) = uri
        .split_once("://")
        .ok_or_else(|| format!("unsupported URI scheme: {uri}"))?;
    if scheme != "rigorix" {
        return Err(format!("unsupported URI scheme: {scheme}"));
    }

    if let Some(exec_id) = rest.strip_prefix("audit/") {
        if exec_id.is_empty() || exec_id.contains('/') {
            return Err(format!("malformed audit resource URI: {uri}"));
        }
        let input = rigorix_mcp::audit_tools::application::dto::ReadAuditInput {
            execution_id: exec_id.to_string(),
            format: None,
        };
        let output = app_state()
            .read_audit_handler
            .handle(input)
            .await
            .map_err(|e| e.to_string())?;
        return output
            .content
            .first()
            .map(|c| c.text.clone())
            .ok_or_else(|| "audit handler returned no content".to_string());
    }

    if let Some(name) = rest.strip_prefix("templates/") {
        if name.is_empty() || name.contains('/') {
            return Err(format!("malformed template resource URI: {uri}"));
        }
        let input = rigorix_mcp::template_tools::domain::value::GetTemplateInput {
            name: name.to_string(),
            format: Some("json".to_string()),
        };
        let output = app_state()
            .get_template_handler
            .handle(&input)
            .await
            .map_err(|e| e.to_string())?;
        return output
            .content
            .first()
            .map(|c| c.text.clone())
            .ok_or_else(|| "template handler returned no content".to_string());
    }

    Err(format!("unknown Rigorix resource: {uri}"))
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

    match name {
        "rigorix_introduction" => {
            let text = concat!(
                "You are using Rigorix, an AI code-governance engine that plans, ",
                "executes, and audits multi-step work against a frozen contract.\n\n",
                "Key capabilities:\n",
                "  • rigorix_list_templates / rigorix_get_template — inspect plan templates\n",
                "  • rigorix_validate_plan — check a plan against enforcement policies\n",
                "  • rigorix_run — execute a template's DAG through the engine\n",
                "  • rigorix_approve_execution — human sign-off when a step requires it\n",
                "    (plans may mark steps requires_approval: true; execution pauses until approved)\n",
                "  • rigorix_check_enforcement — current enforcement status and budget\n",
                "  • rigorix_get_execution_status / rigorix_get_audit_log — inspect evidence\n\n",
                "All execution is gated: budgets, safety caps, tool policy, and (when ",
                "configured) a permission mode (read_only / workspace_write / ",
                "dangerous_full_access). Every audit event is timestamped and, when a ",
                "signing key is configured, HMAC-signed for tamper-evident evidence.\n\n",
                "Start by listing templates, then run one with rigorix_run.",
            );
            let result = serde_json::json!({
                "description": "Introduction to Rigorix tool usage",
                "messages": [
                    {
                        "role": "user",
                        "content": { "type": "text", "text": text }
                    }
                ]
            });
            JsonRpcMessage::success(id.clone(), result)
        }
        _ => JsonRpcMessage::error(
            id.clone(),
            JsonRpcError::internal_error(format!("Prompt '{}' not found", name)),
        ),
    }
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
                                let _ = writer.write_all(format!("{}\n", serde_json::to_string(&error_msg).unwrap_or_default()).as_bytes()).await;
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
    // Initialize tracing via the engine's centralized observability layer.
    // Logs MUST go to stderr: stdout is the JSON-RPC channel for stdio mode.
    let tracing_config = rigorix_engine::observability::TracingConfig {
        write_to_stderr: true,
        ..rigorix_engine::observability::TracingConfig::default()
    };
    if let Err(e) = rigorix_engine::observability::init_tracing(&tracing_config) {
        eprintln!("Failed to initialize tracing: {e}");
    }

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
    let template_repo: SharedTemplateRepository =
        Arc::new(FilesystemTemplateRepository::new(".rigorix/templates"));
    // Same resolution as build_real_engine: rigorix.toml audit_hmac_key or
    // RIGORIX_HMAC_KEY env — used to sign the envelopes read back via
    // rigorix_read_audit so the evidence is real, not a sample.
    let audit_hmac_key = load_toml_config::<Config>(&repo_root, "rigorix.toml")
        .audit_hmac_key
        .or_else(|| std::env::var("RIGORIX_HMAC_KEY").ok())
        .filter(|k| !k.is_empty());
    let _ = APP_STATE.set(AppState::new(engine, template_repo, audit_hmac_key));

    let cancel = CancellationToken::new();

    // Parse args
    let args: Vec<String> = std::env::args().collect();
    let use_sse = args.iter().any(|a| a == "--sse");
    // --bind is accepted for CLI compatibility; the server runs over stdio
    // (GAP-A-10: SSE transport removed).
    let _bind_addr = args
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

    // GAP-A-10: SSE transport removed — the --sse flag never started a real
    // server (it logged 'not fully implemented' and exited). The server now
    // always runs over stdio; --sse is accepted with a deprecation notice so
    // existing invocations do not silently change behavior.
    if use_sse {
        tracing::warn!(
            "SSE transport is not supported (removed); starting in stdio mode. See .pi/architecture/modules/mcp-server.md"
        );
    }
    tracing::info!("Starting MCP Server in stdio mode");
    run_stdio_server(cancel).await;

    tracing::info!("MCP Server shut down gracefully");
}
