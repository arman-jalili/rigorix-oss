//! Host composition — engine/MCP composition root shared by adapters.
//!
//! @canonical .pi/architecture/modules/mcp-server.md
//! Implements: #888 (OSS-C2) — lift the MCP composition (engine facade +
//!   handlers + tool dispatcher) out of the binary so the new
//!   `rigorix-server` native API host can reuse the SAME semantic dispatcher
//!   (`rigorix.execute` -> `rigorix_execute` -> [`AppState::handle_tool_call`])
//!   without duplicating DTOs or orchestration glue.
//!
//! This module is a **pure move** of the composition previously in
//! `src/main.rs`; behavior is unchanged. `main.rs` keeps only MCP protocol
//! framing (initialize/tools/resources/prompts + stdio loop) and delegates to
//! [`app_state()`].
//!
//! Moved here from `main.rs`:
//! - [`AppState`] + [`AppState::new`] + [`AppState::handle_tool_call`]
//! - [`build_real_engine`], [`build_auth_handler`]
//! - [`all_tool_descriptors`], [`error_type_name`]
//! - engine/envelope/permission helpers, [`app_state`] accessor

use std::sync::Arc;
use std::time::Duration;

pub mod error;

use error::HostError;

use rigorix_engine::configuration::domain::config::Config;
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

use rigorix_mcp::enterprise_proxy::domain::entity::SharedEnterpriseProxy;
use rigorix_mcp::enterprise_proxy::domain::value::ProxyConfig;
use rigorix_mcp::enterprise_proxy::infrastructure::EnterpriseProxyImpl;

use rigorix_mcp::mcp_server::application::service::McpToolExecutor;

// =========================================================================
// Composition Root — shared application state
// =========================================================================

/// Shared application state wired from bounded contexts.
pub struct AppState {
    // Enterprise proxy (optional)
    pub enterprise_proxy: Option<SharedEnterpriseProxy>,

    // Engine facade (direct access for approval/sign-off flows)
    engine: SharedEngineFacade,

    // Execution tools
    execute_handler: Box<dyn ExecuteHandler>,
    plan_handler: Box<dyn PlanHandler>,
    validate_plan_handler: Box<dyn ValidatePlanHandler>,
    check_enforcement_handler: Box<dyn CheckEnforcementHandler>,

    // Auth tools (ADR-008) — Some only when an IdP is configured
    // (RIGORIX_IDP_ISSUER + RIGORIX_IDP_CLIENT_ID); None keeps the tools
    // out of the live surface entirely.
    pub auth_handler: Option<Box<dyn rigorix_mcp::auth::interfaces::mcp::AuthToolHandler>>,

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

    // The ENGINE audit service (local signed-trail + delivery) so the
    // approval-resume path can re-dispatch a FINAL envelope — the pause
    // snapshot written to .rigorix/audit must not stand in for a completed
    // runbook (F-20260907-05 follow-up, 2026-09-08).
    engine_audit:
        Option<std::sync::Arc<dyn rigorix_engine::audit::application::service::AuditService>>,

    // ADR-0001 D8 / #900: the engine event bus the native server bridges into
    // its SSE `EventHub`, so `GET /events` is live during real runs.
    engine_event_bus:
        std::sync::Arc<dyn rigorix_engine::event_system::application::service::EventBusService>,

    // HMAC key used to sign envelopes stored for the read_audit cycle.
    audit_hmac_key: Option<String>,

    // Template repository for resolving template_name → plan
    template_repo: SharedTemplateRepository,

    // Live MCP server service — wired with the same handlers so the
    // `mcp_server` library module dispatches to production logic.
    pub mcp_service: Arc<dyn rigorix_mcp::mcp_server::application::service::McpServerService>,
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
    pub fn new(
        engine: SharedEngineFacade,
        template_repo: SharedTemplateRepository,
        audit_hmac_key: Option<String>,
        auth_handler: Option<Box<dyn rigorix_mcp::auth::interfaces::mcp::AuthToolHandler>>,
        engine_audit: Option<
            std::sync::Arc<dyn rigorix_engine::audit::application::service::AuditService>,
        >,
        engine_event_bus: std::sync::Arc<
            dyn rigorix_engine::event_system::application::service::EventBusService,
        >,
    ) -> Self {
        // ── Audit service (in-memory) ──
        let audit_storage = std::sync::Arc::new(
            rigorix_mcp::audit_tools::infrastructure::InMemoryAuditQueryService::new()
                // ADR-016: verify the HMAC of stored audit records on read —
                // a tampered envelope is refused, not returned.
                .with_hmac_key(audit_hmac_key.clone()),
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
            auth_handler,

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
            engine_audit,
            engine_event_bus,
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
    ) -> Result<rigorix_mcp::execution_tools::domain::value::PlanTemplate, HostError> {
        let template = self.template_repo.get(template_name).await.map_err(|e| {
            HostError::invalid_params(format!("Template '{template_name}' not found: {e}"))
        })?;
        let json = serde_json::to_value(&template).unwrap_or_default();
        serde_json::from_value(json).map_err(|e| {
            HostError::invalid_params(format!("Failed to convert template '{template_name}': {e}"))
        })
    }

    /// Active session identity (L1/L2, F-20260907-05): the engine
    /// IdentityRef from the auth handler when an attested claim exists;
    /// None when unauthenticated. Server-side truth — rigorix_run /
    /// rigorix_execute / rigorix_validate_plan inject this; caller-supplied
    /// `author` stays display-only for policy purposes.
    async fn session_identity(&self) -> Option<rigorix_engine::identity::IdentityRef> {
        match &self.auth_handler {
            Some(handler) => handler.current_engine_identity().await,
            None => None,
        }
    }

    /// Route a tool call by name to the appropriate handler.
    pub async fn handle_tool_call(
        &self,
        tool_name: &str,
        params: &serde_json::Value,
    ) -> Result<serde_json::Value, HostError> {
        match tool_name {
            // Execution tools
            "rigorix_execute" => {
                let mut input: rigorix_mcp::execution_tools::application::dto::ExecuteInput =
                    serde_json::from_value(params.clone())
                        .map_err(|e| HostError::invalid_params(format!("Invalid input: {e}")))?;

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
                    return Err(HostError::invalid_params(
                        "Either 'plan' or 'template_name' must be provided",
                    ));
                }

                let template_name_for_audit = template_name
                    .or_else(|| input.plan.as_ref().map(|p| p.name().to_string()))
                    .unwrap_or_else(|| "unknown".to_string());

                // L2 (F-20260907-05): bind the run to the ATTESTED session
                // identity — caller-supplied params never set the policy
                // principal.
                input.identity = self.session_identity().await;

                let result = self
                    .execute_handler
                    .handle(input)
                    .await
                    .map_err(HostError::from)?;
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
                    serde_json::from_value(params.clone())
                        .map_err(|e| HostError::invalid_params(format!("Invalid input: {e}")))?;

                // Load full template (with version, tags, timestamps)
                let template = self
                    .template_repo
                    .get(&input.template_name)
                    .await
                    .map_err(|e| {
                        HostError::invalid_params(format!(
                            "Template '{}' not found: {e}",
                            input.template_name
                        ))
                    })?;

                let result = self
                    .plan_handler
                    .handle(&template)
                    .await
                    .map_err(HostError::from)?;

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
                    identity: self.session_identity().await,
                };

                let result = self
                    .execute_handler
                    .handle(exec_input)
                    .await
                    .map_err(HostError::from)?;
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
                let mut input: rigorix_mcp::execution_tools::application::dto::ValidateInput =
                    serde_json::from_value(params.clone())
                        .map_err(|e| HostError::invalid_params(format!("Invalid input: {e}")))?;
                // L1: preview must refuse require_identity steps the same way
                // the run would — inject the attested session identity.
                input.identity = self.session_identity().await;
                let result = self
                    .validate_plan_handler
                    .handle(input)
                    .await
                    .map_err(HostError::from)?;
                Ok(serde_json::from_str(&result.content[0].text).unwrap_or_default())
            }
            "rigorix_check_enforcement" => {
                let result = self
                    .check_enforcement_handler
                    .handle()
                    .await
                    .map_err(HostError::from)?;
                Ok(serde_json::from_str(&result.content[0].text).unwrap_or_default())
            }
            "rigorix_approve_execution" => {
                use rigorix_mcp::execution_tools::domain::value::ExecutionId;

                let execution_id = params["execution_id"]
                    .as_str()
                    .and_then(|s| uuid::Uuid::parse_str(s).ok())
                    .ok_or_else(|| HostError::invalid_params("Invalid or missing execution_id"))?;
                let step_names: Vec<String> = params["step_names"]
                    .as_array()
                    .ok_or_else(|| HostError::invalid_params("Missing step_names array"))?
                    .iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect();
                if step_names.is_empty() {
                    return Err(HostError::invalid_params(
                        "step_names must contain at least one step name",
                    ));
                }

                // L2 (F-20260907-05): bind the approval to the ATTESTED session
                // identity — never to caller-claimable params. Auth configured
                // + unattested session → refused (approval needs a principal).
                let auth_configured = self.auth_handler.is_some();
                let session_claim = self.session_identity().await;
                let identity =
                    resolve_approval_identity(params, session_claim.as_ref(), auth_configured)
                        .map_err(HostError::invalid_params)?;

                let approval = self
                    .engine
                    .approve_execution(&ExecutionId::from_uuid(execution_id), step_names, identity)
                    .await
                    .map_err(HostError::from)?;

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
                        stored_template.clone(),
                        &self.audit_hmac_key,
                        Some(run_started),
                    );
                    let _ = self.audit_storage.store(envelope);
                    final_state = Some(state.clone());

                    // F-20260907-05 follow-up (2026-09-08): re-dispatch a FINAL
                    // engine envelope so the signed local trail (.rigorix/audit)
                    // reflects the COMPLETED runbook — the pause-point snapshot
                    // written when the run paused must not stand in for a
                    // completed run. Same execution_id => LocalAuditEnvelope
                    // Repository.save overwrites the pause file.
                    if let Some(audit) = &self.engine_audit {
                        use rigorix_engine::audit::application::dto::BuildEnvelopeInput;
                        use rigorix_engine::audit::domain::{EventStatus, ExecutionEventRef};
                        let mut events: Vec<ExecutionEventRef> = state
                            .node_states
                            .values()
                            .filter(|s| s.status == "completed" || s.status == "failed")
                            .map(|s| {
                                let ok = s.status == "completed";
                                ExecutionEventRef {
                                    event_type: if ok {
                                        "node_completed".to_string()
                                    } else {
                                        "node_failed".to_string()
                                    },
                                    summary: format!("step {}", s.node_name),
                                    occurred_at: chrono::Utc::now(),
                                    correlation_id: None,
                                    status: if ok {
                                        EventStatus::Success
                                    } else {
                                        EventStatus::Failure
                                    },
                                    payload: Some(serde_json::json!({
                                        "step_name": s.node_name,
                                    })),
                                }
                            })
                            .collect();
                        // Approval evidence: the attested approver binding.
                        if let Some(claim) = session_claim.as_ref() {
                            for step in approval.approved_steps() {
                                events.push(ExecutionEventRef {
                                    event_type: "approval_recorded".to_string(),
                                    summary: "approval bound to attested identity".to_string(),
                                    occurred_at: chrono::Utc::now(),
                                    correlation_id: None,
                                    status: EventStatus::Success,
                                    payload: Some(serde_json::json!({
                                        "step_name": step,
                                        "approver_id": claim.subject,
                                        "authority": claim.authority,
                                        "decided_at": chrono::Utc::now().to_rfc3339(),
                                    })),
                                });
                            }
                        }
                        let _ = audit
                            .build_and_send(BuildEnvelopeInput {
                                execution_id,
                                template_id: stored_template.clone(),
                                planning_prompt: String::new(),
                                events,
                                source: Some("rigorix_mcp".to_string()),
                                repository: None,
                                author: None,
                                identity: session_claim.clone(),
                                effect_key: None,
                                producer_id: None,
                                sequence: None,
                                prev_hash: None,
                                history_policy: None,
                                total_tokens: 0,
                                duration_ms: state.total_duration_ms,
                                git_commit: None,
                                git_branch: None,
                                model_version: None,
                                planning_prompt_content: None,
                                file_paths: Vec::new(),
                                metadata: None,
                                sign: true,
                                scoring_results: Default::default(),
                            })
                            .await;
                    }
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
                    .map_err(|e| HostError::invalid_params(format!("Invalid input: {e}")))?;
                let result = self
                    .read_audit_handler
                    .handle(input)
                    .await
                    .map_err(HostError::from)?;
                // The handler may return markdown (text format) or JSON — pass
                // the text through, only parsing when it is actually JSON.
                let text = &result.content[0].text;
                Ok(serde_json::from_str(text)
                    .unwrap_or_else(|_| serde_json::Value::String(text.clone())))
            }
            "rigorix_list_audits" => {
                let input = serde_json::from_value(params.clone())
                    .map_err(|e| HostError::invalid_params(format!("Invalid input: {e}")))?;
                let result = self
                    .list_audits_handler
                    .handle(input)
                    .await
                    .map_err(HostError::from)?;
                let text = &result.content[0].text;
                Ok(serde_json::from_str(text)
                    .unwrap_or_else(|_| serde_json::Value::String(text.clone())))
            }
            "rigorix_audit_summary" => {
                let input = serde_json::from_value(params.clone())
                    .map_err(|e| HostError::invalid_params(format!("Invalid input: {e}")))?;
                let result = self
                    .audit_summary_handler
                    .handle(input)
                    .await
                    .map_err(HostError::from)?;
                Ok(serde_json::from_str(&result.content[0].text).unwrap_or_default())
            }

            // Template tools
            "rigorix_list_templates" => {
                let filter = serde_json::from_value(params.clone())
                    .map_err(|e| HostError::invalid_params(format!("Invalid input: {e}")))?;
                let result = self
                    .list_templates_handler
                    .handle(&filter)
                    .await
                    .map_err(HostError::from)?;
                Ok(serde_json::from_str(&result.content[0].text).unwrap_or_default())
            }
            "rigorix_get_template" => {
                let input = serde_json::from_value(params.clone())
                    .map_err(|e| HostError::invalid_params(format!("Invalid input: {e}")))?;
                let result = self
                    .get_template_handler
                    .handle(&input)
                    .await
                    .map_err(HostError::from)?;
                Ok(serde_json::from_str(&result.content[0].text).unwrap_or_default())
            }
            "rigorix_create_template" => {
                let input = serde_json::from_value(params.clone())
                    .map_err(|e| HostError::invalid_params(format!("Invalid input: {e}")))?;
                let result = self
                    .create_template_handler
                    .handle(&input)
                    .await
                    .map_err(HostError::from)?;
                Ok(serde_json::from_str(&result.content[0].text).unwrap_or_default())
            }
            "rigorix_validate_template" => {
                let input = serde_json::from_value(params.clone())
                    .map_err(|e| HostError::invalid_params(format!("Invalid input: {e}")))?;
                let result = self
                    .validate_template_handler
                    .handle(&input)
                    .await
                    .map_err(HostError::from)?;
                Ok(serde_json::from_str(&result.content[0].text).unwrap_or_default())
            }

            // Usage guide tool
            "rigorix_get_usage_guide" => {
                Ok(rigorix_mcp::usage_guide::interfaces::mcp::handle_get_usage_guide())
            }

            // Auth tools (ADR-008) — optional; present only when an IdP is
            // configured (RIGORIX_IDP_ISSUER + RIGORIX_IDP_CLIENT_ID).
            "rigorix_auth_login" | "rigorix_auth_status" | "rigorix_auth_logout" => {
                let Some(handler) = &self.auth_handler else {
                    return Err(HostError::not_authenticated(
                        "auth tools are not configured — set RIGORIX_IDP_ISSUER and RIGORIX_IDP_CLIENT_ID (see .pi/architecture/modules/auth.md)",
                    ));
                };
                let result = match tool_name {
                    "rigorix_auth_login" => handler.handle_auth_login(params.clone()).await,
                    "rigorix_auth_status" => handler.handle_auth_status(params.clone()).await,
                    _ => handler.handle_auth_logout(params.clone()).await,
                };
                result.map_err(HostError::from)
            }

            _ => Err(HostError::method_not_found(tool_name)),
        }
    }

    /// The engine event bus (ADR-0001 D8 / #900). The native server bridges
    /// this into its SSE `EventHub` so `GET /events` emits live run events —
    /// the SAME bus the orchestrator/executor publish into (never a second
    /// source).
    pub fn engine_event_bus(
        &self,
    ) -> std::sync::Arc<dyn rigorix_engine::event_system::application::service::EventBusService>
    {
        std::sync::Arc::clone(&self.engine_event_bus)
    }
}

// =========================================================================
// Real Engine Builder — constructs EngineFacadeImpl from rigorix-engine services
// =========================================================================

// ── Helpers ──────────────────────────────────────────────────────────────

/// L2 approval binding (F-20260907-05): resolve the identity recorded on an
/// approval from the ATTESTED session, never from caller-claimable params.
///
/// - No auth configured (legacy/local flows): caller-supplied params pass
///   through unchanged (no IdP to attest against).
/// - Auth configured + no attested session: REFUSE — an approval must bind
///   to a real attested principal (run rigorix_auth_login).
/// - Auth configured + attested session: approver_id = the claim's subject
///   (server truth), token_claims_ref = a server-derived verified reference
///   (`oidc-device:<issuer>#<subject>`). Caller-supplied approver_id /
///   token_claims_ref are ignored as identity; `authority` remains
///   display-only metadata.
fn resolve_approval_identity(
    params: &serde_json::Value,
    claim: Option<&rigorix_engine::identity::IdentityRef>,
    auth_configured: bool,
) -> Result<Option<rigorix_mcp::execution_tools::domain::value::ApprovalIdentity>, String> {
    use rigorix_mcp::execution_tools::domain::value::ApprovalIdentity;

    if !auth_configured {
        let identity = ApprovalIdentity {
            approver_id: params["approver_id"].as_str().map(|s| s.to_string()),
            authority: params["authority"].as_str().map(|s| s.to_string()),
            token_claims_ref: params["token_claims_ref"].as_str().map(|s| s.to_string()),
        };
        return Ok((identity.approver_id.is_some()
            || identity.authority.is_some()
            || identity.token_claims_ref.is_some())
        .then_some(identity));
    }

    let claim = claim.ok_or_else(|| {
        "approval requires an attested identity — run rigorix_auth_login first".to_string()
    })?;
    Ok(Some(ApprovalIdentity {
        // Server truth: the attested subject — never the caller's string.
        approver_id: Some(claim.subject.clone()),
        // Display-only provenance (e.g. "device-flow:…") — captured fact.
        authority: params["authority"].as_str().map(|s| s.to_string()),
        // Verified reference derived from the attested claim.
        token_claims_ref: Some(format!("oidc-device:{}#{}", claim.issuer, claim.subject)),
    }))
}

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
pub fn load_toml_config<T: serde::de::DeserializeOwned + Default>(
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
/// ADR-008: optional OIDC device-flow surface — `rigorix_auth_login`,
/// `rigorix_auth_status`, `rigorix_auth_logout`.
///
/// Enabled by the canonical env surface (`RIGORIX_IDP_ISSUER` +
/// `RIGORIX_IDP_CLIENT_ID`, mirrors `.rigorix/auth.toml` `[auth]`; optional
/// `RIGORIX_IDP_CLIENT_SECRET`, `RIGORIX_IDP_ACCESS_TOKEN_TTL_SECS`).
/// Returns `None` when unconfigured or any piece fails to initialize —
/// auth tools then stay out of the live surface (no behavior change).
///
/// Keychain: OS keychain by default; headless/CI can opt into the explicit
/// plaintext fallback with `RIGORIX_AUTH_PLAINTEXT_DIR` (never automatic).
pub async fn build_auth_handler()
-> Option<Box<dyn rigorix_mcp::auth::interfaces::mcp::AuthToolHandler>> {
    use rigorix_mcp::auth::application::factory::{AuthServiceFactory, AuthServiceFactoryImpl};
    use rigorix_mcp::auth::domain::config::IdpConfig;
    use rigorix_mcp::auth::infrastructure::{
        HttpIdpClient, InMemoryTokenProvider, KeychainStoreImpl,
    };
    use rigorix_mcp::auth::interfaces::mcp::AuthToolHandlerImpl;
    use std::sync::Arc;

    let issuer = std::env::var("RIGORIX_IDP_ISSUER").ok()?;
    let client_id = std::env::var("RIGORIX_IDP_CLIENT_ID").ok()?;
    if issuer.trim().is_empty() || client_id.trim().is_empty() {
        tracing::info!("auth: RIGORIX_IDP_* present but empty — rigorix_auth_* disabled");
        return None;
    }
    let client_secret = std::env::var("RIGORIX_IDP_CLIENT_SECRET").ok();
    let ttl = std::env::var("RIGORIX_IDP_ACCESS_TOKEN_TTL_SECS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok());
    let config = match IdpConfig::new(issuer, client_id, client_secret, ttl) {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("auth: invalid IdP config — rigorix_auth_* disabled ({e})");
            return None;
        }
    };
    let idp: Arc<dyn rigorix_mcp::auth::infrastructure::idp_client::IdpClient> =
        match HttpIdpClient::new(config.issuer()) {
            Ok(c) => Arc::new(c),
            Err(e) => {
                tracing::warn!("auth: IdP client init failed — rigorix_auth_* disabled ({e})");
                return None;
            }
        };
    // Keychain: OS default; explicit plaintext dir opt-in for headless/CI.
    let keychain: Arc<dyn rigorix_mcp::auth::infrastructure::keychain_store::KeychainStore> =
        match std::env::var("RIGORIX_AUTH_PLAINTEXT_DIR")
            .ok()
            .filter(|d| !d.trim().is_empty())
        {
            Some(dir) => match KeychainStoreImpl::plaintext(dir) {
                Ok(k) => {
                    tracing::warn!(
                        "auth: explicit PLAINTEXT keychain fallback enabled (RIGORIX_AUTH_PLAINTEXT_DIR) — degraded mode"
                    );
                    Arc::new(k)
                }
                Err(e) => {
                    tracing::warn!(
                        "auth: plaintext store init failed ({e}) — rigorix_auth_* disabled"
                    );
                    return None;
                }
            },
            None => match KeychainStoreImpl::keychain() {
                Ok(k) => Arc::new(k),
                Err(e) => {
                    tracing::warn!(
                        "auth: OS keychain unavailable ({e}) — set RIGORIX_AUTH_PLAINTEXT_DIR for a headless/CI run, or rigorix_auth_* stays disabled"
                    );
                    return None;
                }
            },
        };
    let tokens: Arc<dyn rigorix_mcp::auth::infrastructure::token_provider::TokenProvider> =
        Arc::new(InMemoryTokenProvider::new());
    // ADR-012 seam: the engine's identity attestation service. When the IdP
    // is reachable we wire the JWKS-backed verifier so device-flow tokens
    // attest with source = idp_token (real verification); if discovery fails
    // or no jwks_uri is advertised we fall back to the offline NullVerifier
    // (claims degrade to Unverified — explicit, never silent; L1 identity
    // gate then refuses require_identity steps, which is the safe posture).
    let attestation: Arc<
        dyn rigorix_engine::identity::application::service::IdentityAttestationService,
    > = {
        let verifier: Box<dyn rigorix_engine::identity::infrastructure::TokenVerifier> = match idp
            .discover()
            .await
        {
            Ok(meta) => match meta.jwks_uri {
                Some(jwks_url) => Box::new(
                    rigorix_engine::identity::infrastructure::JwksVerifier::new(jwks_url),
                ),
                None => {
                    tracing::warn!(
                        "auth: IdP discovery returned no jwks_uri — claims degrade to unverified"
                    );
                    Box::new(rigorix_engine::identity::infrastructure::NullVerifier::new())
                }
            },
            Err(e) => {
                tracing::warn!("auth: IdP discovery failed ({e}) — claims degrade to unverified");
                Box::new(rigorix_engine::identity::infrastructure::NullVerifier::new())
            }
        };
        Arc::new(
            rigorix_engine::identity::application::service_impl::IdentityAttestationServiceImpl::with_verifier(
                verifier,
            ),
        )
    };

    match AuthServiceFactoryImpl::new()
        .create(config, idp, keychain, tokens, attestation)
        .await
    {
        Ok(service) => {
            tracing::info!("auth: rigorix_auth_login/status/logout ENABLED (ADR-008)");
            Some(Box::new(AuthToolHandlerImpl::new(service)))
        }
        Err(e) => {
            tracing::warn!("auth: service composition failed — rigorix_auth_* disabled ({e})");
            None
        }
    }
}

pub async fn build_real_engine(
    repo_root: &str,
) -> Result<
    (
        SharedEngineFacade,
        Option<std::sync::Arc<dyn rigorix_engine::audit::application::service::AuditService>>,
        std::sync::Arc<dyn rigorix_engine::event_system::application::service::EventBusService>,
    ),
    Box<dyn std::error::Error + Send + Sync>,
> {
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
                        require_identity: false,
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
                        require_identity: false,
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
    // ADR-013 R3/R2: operator-authored `.rigorix/sequence-policy.toml` gates
    // the dispatch prefix (executor R3) and the plan (orchestrator R2 below).
    let sequence_policy =
        rigorix_engine::execution_engine::application::factory::SequencePolicySetup::from_env(
            std::path::Path::new(repo_root),
        );
    // ── Event bus (shared: the executor publishes into it AND the
    //    orchestrator drains it for the audit envelope — a split bus makes
    //    engine envelopes evidence-empty) ──
    let event_bus: Arc<dyn EventBusService> =
        Arc::new(EventBusServiceImpl::new(EventBusConfig::default()));

    let execution_service: Arc<dyn ParallelExecutionService> = Arc::from(
        ParallelExecutionFactoryImpl::new()
            .create(ParallelExecutionFactoryConfig {
                permission_enforcer,
                hook_runner,
                event_bus: Some(Arc::clone(&event_bus)),
                approval_binding: rigorix_engine::execution_engine::application::factory::ApprovalBindingSetup::from_env(std::path::Path::new(repo_root)),
                sequence_policy: sequence_policy.clone(),
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
    let audit_service: Arc<dyn AuditService> = {
        let mut service = AuditServiceImpl::new(
            Box::new(AuditEnvelopeFactoryImpl::new(hmac_key)),
            audit_sender,
            Box::new(AuditQueueImpl::default()),
            audit_url.is_some(),
        );
        // R7: persist every built envelope to `<repo_root>/.rigorix/audit` —
        // the signed trail that cross-run policy reads (SequencePolicySetup::
        // from_env wires the EnvelopeHistoryAdapter over the same directory).
        let audit_dir = std::path::PathBuf::from(repo_root)
            .join(".rigorix")
            .join("audit");
        if std::fs::create_dir_all(&audit_dir).is_ok() {
            service = service.with_local_repository(std::sync::Arc::new(
                rigorix_engine::audit::infrastructure::LocalAuditEnvelopeRepository::new(audit_dir),
            ));
        }
        Arc::new(service)
    };

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
        .with_event_bus(Arc::clone(&event_bus))
        .with_audit_service(audit_service.clone())
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

    if let Some(policy) = sequence_policy {
        builder = builder.with_sequence_policy(policy);
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

    Ok((Arc::new(engine), Some(audit_service), event_bus))
}

// Build the intent formatter — LLM-based when provider env vars are set,
// JSON fallback otherwise.
// =========================================================================
// Tool descriptors — all OSS tools
// =========================================================================

/// Returns the list of all registered OSS MCP tool descriptors.
pub fn all_tool_descriptors() -> Vec<serde_json::Value> {
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
pub fn error_type_name(
    e: &rigorix_mcp::enterprise_proxy::domain::error::ProxyError,
) -> &'static str {
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
pub static APP_STATE: std::sync::OnceLock<AppState> = std::sync::OnceLock::new();

/// Build and install the global host state from a repo root.
///
/// Shared composition entry point for the stdio MCP binary and the native API
/// server (#888 OSS-C2): both adapters dispatch through the SAME
/// [`AppState::handle_tool_call`]. Idempotent — a second call is a no-op.
///
/// # Errors
/// Returns the engine-build error when the real engine facade cannot be
/// constructed.
pub async fn init_host(repo_root: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let (engine, engine_audit, engine_event_bus) = build_real_engine(repo_root).await?;
    let template_repo: SharedTemplateRepository =
        Arc::new(FilesystemTemplateRepository::new(".rigorix/templates"));
    // Same resolution as build_real_engine: rigorix.toml audit_hmac_key or
    // RIGORIX_HMAC_KEY env — used to sign the envelopes read back via
    // rigorix_read_audit so the evidence is real, not a sample.
    let audit_hmac_key = load_toml_config::<Config>(repo_root, "rigorix.toml")
        .audit_hmac_key
        .or_else(|| std::env::var("RIGORIX_HMAC_KEY").ok())
        .filter(|k| !k.is_empty());
    let _ = APP_STATE.set(AppState::new(
        engine,
        template_repo,
        audit_hmac_key,
        build_auth_handler().await,
        engine_audit,
        engine_event_bus,
    ));
    Ok(())
}

pub fn app_state() -> &'static AppState {
    APP_STATE
        .get()
        .expect("AppState not initialized — call init_app_state() in main()")
}

#[cfg(test)]
mod approval_binding_tests {
    use super::*;
    use rigorix_engine::identity::{IdentityRef, IdentitySource};

    fn claim(subject: &str) -> IdentityRef {
        IdentityRef {
            subject: subject.to_string(),
            issuer: "http://idp/realms/rigorix".to_string(),
            source: IdentitySource::IdpToken,
            authority: None,
            expires_at: None,
        }
    }

    fn params(approver: Option<&str>) -> serde_json::Value {
        let mut v = serde_json::json!({});
        if let Some(a) = approver {
            v["approver_id"] = serde_json::Value::String(a.to_string());
        }
        v
    }

    #[test]
    fn no_auth_legacy_passthrough() {
        let p = params(Some("organizer@corp.demo"));
        let id = resolve_approval_identity(&p, None, false).unwrap();
        assert_eq!(
            id.unwrap().approver_id.as_deref(),
            Some("organizer@corp.demo")
        );
    }

    #[test]
    fn auth_configured_without_session_refuses() {
        let p = params(Some("organizer@corp.demo"));
        let err = resolve_approval_identity(&p, None, true).unwrap_err();
        assert!(err.contains("rigorix_auth_login"), "unexpected: {err}");
    }

    #[test]
    fn auth_binds_subject_and_ignores_caller_approver() {
        let p = params(Some("organizer@corp.demo")); // spoof attempt
        let id = resolve_approval_identity(&p, Some(&claim("sub-123")), true)
            .unwrap()
            .expect("bound");
        assert_eq!(
            id.approver_id.as_deref(),
            Some("sub-123"),
            "server truth wins"
        );
        let ref_ = id.token_claims_ref.unwrap();
        assert!(
            ref_.contains("http://idp/realms/rigorix") && ref_.contains("sub-123"),
            "verified ref: {ref_}"
        );
    }

    #[test]
    fn auth_authority_stays_display_only() {
        let mut p = params(Some("organizer@corp.demo"));
        p["authority"] = serde_json::json!("device-flow:kc");
        let id = resolve_approval_identity(&p, Some(&claim("sub-1")), true)
            .unwrap()
            .unwrap();
        assert_eq!(id.authority.as_deref(), Some("device-flow:kc"));
        assert_eq!(id.approver_id.as_deref(), Some("sub-1"));
    }
}

/// Catalog drift: every MCP tool maps 1:1 to a frozen `rigorix.*` catalog
/// method (ADR-0001 D9). Runs only when `RIGORIX_SDK_SCHEMAS` points at a
/// rigorix-sdk checkout (same env as the engine conformance job); the OSS CI
/// conformance job sets it and runs this test.
#[cfg(test)]
mod catalog_drift_tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn mcp_tools_map_one_to_one_to_the_frozen_catalog() {
        let Ok(dir) = std::env::var("RIGORIX_SDK_SCHEMAS") else {
            eprintln!("RIGORIX_SDK_SCHEMAS unset — skipping catalog drift check");
            return;
        };
        let path = std::path::Path::new(&dir).join("api/catalog.json");
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        let catalog: serde_json::Value = serde_json::from_str(&text).expect("catalog parses");

        let catalog_tools: BTreeSet<String> = catalog["methods"]
            .as_array()
            .expect("methods array")
            .iter()
            .filter_map(|m| m["mcpTool"].as_str().map(str::to_string))
            .collect();

        // The stdio server's tool surface = the core descriptors + the auth
        // descriptors (registered separately). Enterprise proxy tools are a
        // seam, not catalog methods.
        let mut mcp_tools: BTreeSet<String> = all_tool_descriptors()
            .iter()
            .filter_map(|d| d["name"].as_str().map(str::to_string))
            .collect();
        for d in [
            rigorix_mcp::auth::interfaces::mcp::rigorix_auth_login_tool_descriptor(),
            rigorix_mcp::auth::interfaces::mcp::rigorix_auth_status_tool_descriptor(),
            rigorix_mcp::auth::interfaces::mcp::rigorix_auth_logout_tool_descriptor(),
        ] {
            if let Some(name) = d["name"].as_str() {
                mcp_tools.insert(name.to_string());
            }
        }

        assert_eq!(
            mcp_tools, catalog_tools,
            "MCP tools must map 1:1 to catalog methods (ADR-0001 D9); catalog={catalog_tools:?} mcp={mcp_tools:?}"
        );
    }
}

/// Resolve a rigorix:// resource URI to its text payload.
pub async fn resolve_resource(uri: &str) -> Result<String, String> {
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
