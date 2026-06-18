//! Implementation of `OrchestratorService`.
//!
//! @canonical .pi/architecture/modules/orchestrator.md#orchestrator-impl
//! Implements: Issue #339 — OrchestratorService concrete implementation
//! Issue: #339

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::orchestrator::domain::record::EventInfoStatus;
use crate::orchestrator::domain::record::ExecutionContext;
use crate::orchestrator::domain::record::ExecutionEventInfo;
use crate::orchestrator::domain::record::ExecutionRecord;
use crate::orchestrator::domain::record::ExecutionStatus;
use crate::orchestrator::domain::record::PlanningMetadata;
use crate::orchestrator::domain::record::TaskResult;
use crate::orchestrator::domain::record::TaskStatus;
use crate::orchestrator::domain::{OrchestratorConfig, OrchestratorError};

use super::dto::{
    CancelInput, CancelOutput, NodeState, PlanOnlyInput, PlanOnlyOutput, RunInput, RunOutput,
    StatusOutput,
};
use super::service::OrchestratorService;

// DTO submodule aliases
use crate::audit::application as audit_app;
use crate::budget_tracking::application as budget_app;
use crate::cancellation::application as cancel_app;
use crate::code_graph::application::CodeGraphService as CodeGraphServiceTrait;
use crate::code_graph::application::service::CodeGraphFormatter as CodeGraphFormatterTrait;
use crate::code_graph::application::service_impl::CodeGraphFormatterImpl;
use crate::event_system::application as event_app;
use crate::execution_engine::application::{dto as exec_dto, service as exec_svc};
use crate::planning::application::dto as planning_dto;
use crate::state_persistence::application::{dto as state_dto, service as state_svc};

pub struct OrchestratorServiceImpl {
    config: OrchestratorConfig,
    planning_pipeline: Arc<dyn crate::planning::application::PlanningPipelineService>,
    execution_service: Arc<dyn exec_svc::ParallelExecutionService>,
    state_manager: Arc<dyn state_svc::StateManagerService>,
    cancellation_service: Arc<dyn cancel_app::CancellationService>,
    event_bus: Arc<dyn event_app::EventBusService>,
    #[allow(dead_code)]
    audit_service: Option<Arc<dyn audit_app::AuditService>>,
    #[allow(dead_code)]
    budget_service: Arc<dyn budget_app::LlmBudgetService>,
    #[allow(dead_code)]
    code_graph_service: Option<Arc<dyn CodeGraphServiceTrait>>,
    current_execution: Arc<RwLock<Option<CurrentExecutionState>>>,
}

#[derive(Debug, Clone)]
struct CurrentExecutionState {
    execution_id: Uuid,
    status: ExecutionStatus,
    nodes: Vec<NodeState>,
    #[allow(dead_code)]
    started_at: chrono::DateTime<chrono::Utc>,
}

impl OrchestratorServiceImpl {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        config: OrchestratorConfig,
        planning_pipeline: Arc<dyn crate::planning::application::PlanningPipelineService>,
        execution_service: Arc<dyn exec_svc::ParallelExecutionService>,
        state_manager: Arc<dyn state_svc::StateManagerService>,
        cancellation_service: Arc<dyn cancel_app::CancellationService>,
        event_bus: Arc<dyn event_app::EventBusService>,
        audit_service: Option<Arc<dyn audit_app::AuditService>>,
        budget_service: Arc<dyn budget_app::LlmBudgetService>,
        code_graph_service: Option<Arc<dyn CodeGraphServiceTrait>>,
    ) -> Self {
        Self {
            config,
            planning_pipeline,
            execution_service,
            state_manager,
            cancellation_service,
            event_bus,
            audit_service,
            budget_service,
            code_graph_service,
            current_execution: Arc::new(RwLock::new(None)),
        }
    }

    #[cfg(test)]
    pub fn default_test() -> Self {
        Self::new(
            OrchestratorConfig::default(),
            Arc::new(mocks::MockPlanningService::new()),
            Arc::new(mocks::MockExecutionService),
            Arc::new(mocks::MockStateService::new()),
            Arc::new(mocks::MockCancellationService),
            Arc::new(mocks::MockEventBusService::new()),
            None,
            Arc::new(mocks::MockBudgetService),
            None,
        )
    }

    fn gen_id(&self) -> Uuid {
        Uuid::new_v4()
    }

    /// Build a module dependency graph string from the repo root.
    ///
    /// Uses CodeGraphBuilder to scan the workspace and CodeGraphFormatter
    /// to produce compact output. Returns None if CodeGraphService is not
    /// configured or if any step fails (non-fatal — the pipeline continues
    /// without module deps).
    async fn build_module_deps(&self, repo_root: &str) -> Option<String> {
        let code_graph_service = self.code_graph_service.as_ref()?.clone();
        let root = std::path::PathBuf::from(repo_root);
        if !root.exists() {
            return None;
        }

        // 1. Use CodeGraphBuilder to scan the workspace
        let extensions = vec![
            "rs".to_string(),
            "ts".to_string(),
            "tsx".to_string(),
            "js".to_string(),
            "py".to_string(),
        ];
        let builder = crate::code_graph::application::builder::CodeGraphBuilder::new(
            code_graph_service.clone(),
            vec![root.clone()],
            extensions,
            false,
        );
        let build_out = builder.build().await.ok()?;

        // 2. Format as compact citations (FastContext <final_answer> pattern)
        let formatter = CodeGraphFormatterImpl::new();
        let formatted = CodeGraphFormatterTrait::format(
            &formatter,
            crate::code_graph::application::dto::FormatGraphInput {
                graph: build_out.graph,
                format: crate::code_graph::application::dto::OutputFormat::Compact,
                include_metadata: false,
            },
        )
        .await
        .ok()?;

        Some(formatted.output)
    }

    #[allow(clippy::too_many_arguments)]
    fn build_record(
        &self,
        execution_id: Uuid,
        started_at: chrono::DateTime<chrono::Utc>,
        status: ExecutionStatus,
        planning_meta: Option<PlanningMetadata>,
        task_results: Vec<TaskResult>,
        context: ExecutionContext,
        events: Vec<ExecutionEventInfo>,
    ) -> ExecutionRecord {
        let now = chrono::Utc::now();
        let duration_ms = now
            .signed_duration_since(started_at)
            .num_milliseconds()
            .max(0) as u64;
        let completed_at = Some(now);
        ExecutionRecord {
            execution_id,
            planning: planning_meta.unwrap_or(PlanningMetadata {
                template_id: String::new(),
                confidence: 0.0,
                llm_calls: 0,
                total_tokens: 0,
                prompt_hash: String::new(),
                generated_toml: None,
                node_order: vec![],
            }),
            task_results,
            events,
            context,
            started_at,
            completed_at,
            duration_ms,
            status,
        }
    }

    fn make_pending_state(execution_id: Uuid) -> state_dto::SaveStateInput {
        let mut state =
            crate::state_persistence::domain::ExecutionState::new(execution_id, String::new());
        state.status = crate::state_persistence::domain::ExecutionStatus::Pending;
        state_dto::SaveStateInput { state }
    }

    fn make_final_state(execution_id: Uuid, status: ExecutionStatus) -> state_dto::SaveStateInput {
        use crate::state_persistence::domain::ExecutionStatus as SpStatus;
        let sp_status = match status {
            ExecutionStatus::Completed => SpStatus::Completed,
            ExecutionStatus::PartialFailure | ExecutionStatus::Failed => SpStatus::Failed,
            ExecutionStatus::Cancelled => SpStatus::Cancelled,
        };
        let mut state =
            crate::state_persistence::domain::ExecutionState::new(execution_id, String::new());
        state.status = sp_status;
        state.completed_at = Some(chrono::Utc::now());
        state_dto::SaveStateInput { state }
    }

    fn planning_started_event(execution_id: Uuid, intent: String) -> event_app::PublishEventInput {
        event_app::PublishEventInput {
            event: crate::event_system::domain::ExecutionEvent::PlanningStarted {
                execution_id,
                intent,
                timestamp: chrono::Utc::now(),
            },
        }
    }

    fn planning_completed_event(
        execution_id: Uuid,
        pr: &crate::planning::domain::result::PlanningResult,
    ) -> event_app::PublishEventInput {
        event_app::PublishEventInput {
            event: crate::event_system::domain::ExecutionEvent::PlanningCompleted {
                execution_id,
                template_id: pr.template_id.clone(),
                confidence: pr.confidence,
                parameters: std::collections::HashMap::new(),
                timestamp: chrono::Utc::now(),
            },
        }
    }

    fn planning_meta(
        pr: &crate::planning::domain::result::PlanningResult,
        graph: Option<&crate::dag_engine::domain::TaskGraph>,
    ) -> PlanningMetadata {
        let node_order = match graph {
            Some(g) => match g.topological_order() {
                Some(order) => order
                    .iter()
                    .map(|id| {
                        g.get_node(*id)
                            .map(|n| n.name.clone())
                            .unwrap_or_else(|| id.to_string())
                    })
                    .collect::<Vec<_>>(),
                None => vec![],
            },
            None => vec![],
        };

        PlanningMetadata {
            template_id: pr.template_id.clone(),
            confidence: pr.confidence,
            llm_calls: pr.llm_calls_used,
            total_tokens: pr.llm_tokens_used,
            prompt_hash: pr.planning_hash.0.clone(),
            generated_toml: pr.generated_toml.clone(),
            node_order,
        }
    }
}

#[async_trait]
impl OrchestratorService for OrchestratorServiceImpl {
    #[tracing::instrument(skip_all)]
    async fn run(&self, input: RunInput) -> Result<RunOutput, OrchestratorError> {
        let execution_id = self.gen_id();
        let started_at = chrono::Utc::now();
        tracing::info!(%execution_id, "Starting orchestrator run");

        // Init current execution state
        *self.current_execution.write().await = Some(CurrentExecutionState {
            execution_id,
            status: ExecutionStatus::Failed,
            nodes: vec![],
            started_at,
        });

        // 1. Publish PlanningStarted
        let _ = self
            .event_bus
            .publish(Self::planning_started_event(
                execution_id,
                input.intent.clone(),
            ))
            .await;

        // 2. Build module dependency graph if CodeGraphService is available
        let module_deps = self.build_module_deps(&input.repo_root).await;

        // 3. Run planning pipeline
        let plan_out = self
            .planning_pipeline
            .plan_with_graph(planning_dto::PlanWithGraphInput {
                intent: crate::planning::domain::intent::UserIntent::new(
                    input.intent.clone(),
                    Some(execution_id),
                ),
                execution_id: Some(execution_id),
                enable_generator_fallback: true,
                skip_validation: false,
                repo_root: input.repo_root.clone(),
                module_deps,
            })
            .await
            .map_err(|e| OrchestratorError::PlanningFailed {
                detail: e.to_string(),
                intent: input.intent.clone(),
            })?;

        // 3. Publish PlanningCompleted
        let _ = self
            .event_bus
            .publish(Self::planning_completed_event(
                execution_id,
                &plan_out.planning_result,
            ))
            .await;

        let pmeta = Self::planning_meta(&plan_out.planning_result, Some(&plan_out.graph));

        // 4. Save initial state
        self.state_manager
            .save_state(Self::make_pending_state(execution_id))
            .await
            .map_err(|e| OrchestratorError::StatePersistenceFailed {
                detail: e.to_string(),
                state: "Pending".into(),
            })?;

        // 5. Execute DAG — pass the graph from planning
        let task_results = self
            .execution_service
            .execute_graph(exec_dto::ExecuteGraphInput {
                dag_id: execution_id,
                graph: Some(plan_out.graph),
                config_override: None,
            })
            .await
            .map_err(|e| OrchestratorError::ExecutionFailed {
                detail: e.to_string(),
                nodes_completed: 0,
                nodes_remaining: 0,
            })
            .map(|o| {
                o.result
                    .node_results
                    .into_values()
                    .map(|nr| TaskResult {
                        node_id: nr.node_id.to_string(),
                        node_name: nr.node_name,
                        status: if nr.success {
                            TaskStatus::Success
                        } else {
                            TaskStatus::Failure
                        },
                        duration_ms: nr.duration_ms,
                        output: nr.output,
                        error: nr.error,
                        retry_attempts: nr.retry_attempts as u32,
                        tool_used: None,
                    })
                    .collect::<Vec<_>>()
            })?;

        // 6. Determine final status
        let final_status = if task_results.is_empty() {
            ExecutionStatus::Completed
        } else {
            let f = task_results.iter().any(|t| t.status == TaskStatus::Failure);
            let s = task_results.iter().any(|t| t.status == TaskStatus::Success);
            if f && s {
                ExecutionStatus::PartialFailure
            } else if f {
                ExecutionStatus::Failed
            } else {
                ExecutionStatus::Completed
            }
        };

        // 7. Save final state
        self.state_manager
            .save_state(Self::make_final_state(execution_id, final_status))
            .await
            .map_err(|e| OrchestratorError::StatePersistenceFailed {
                detail: e.to_string(),
                state: format!("{final_status:?}"),
            })?;

        // 8. Drain events
        let events = self
            .event_bus
            .drain_persisted(event_app::DrainPersistedInput { clear: true })
            .await
            .map(|o| {
                o.events
                    .into_iter()
                    .map(|pe| {
                        let ts = match &pe.event {
                            crate::event_system::domain::ExecutionEvent::PlanningStarted {
                                timestamp,
                                ..
                            } => *timestamp,
                            crate::event_system::domain::ExecutionEvent::PlanningCompleted {
                                timestamp,
                                ..
                            } => *timestamp,
                            crate::event_system::domain::ExecutionEvent::NodeStarted {
                                timestamp,
                                ..
                            } => *timestamp,
                            crate::event_system::domain::ExecutionEvent::NodeCompleted {
                                timestamp,
                                ..
                            } => *timestamp,
                            crate::event_system::domain::ExecutionEvent::NodeFailed {
                                timestamp,
                                ..
                            } => *timestamp,
                            crate::event_system::domain::ExecutionEvent::NodeRetrying {
                                timestamp,
                                ..
                            } => *timestamp,
                            crate::event_system::domain::ExecutionEvent::ToolExecuted {
                                timestamp,
                                ..
                            } => *timestamp,
                            crate::event_system::domain::ExecutionEvent::ExecutionCompleted {
                                timestamp,
                                ..
                            } => *timestamp,
                            crate::event_system::domain::ExecutionEvent::ExecutionFailed {
                                timestamp,
                                ..
                            } => *timestamp,
                            crate::event_system::domain::ExecutionEvent::ExecutionCancelled {
                                timestamp,
                                ..
                            } => *timestamp,
                            crate::event_system::domain::ExecutionEvent::BudgetWarning {
                                timestamp,
                                ..
                            } => *timestamp,
                        };
                        ExecutionEventInfo {
                            event_type: pe.event.event_type_name().to_string(),
                            summary: pe.event.event_type_name().to_string(),
                            occurred_at: ts,
                            correlation_id: None,
                            payload: None,
                            status: EventInfoStatus::Info,
                        }
                    })
                    .collect()
            })
            .unwrap_or_default();

        // 9. Build record
        let record = self.build_record(
            execution_id,
            started_at,
            final_status,
            Some(pmeta),
            task_results,
            ExecutionContext {
                repo_root: input.repo_root,
                symbol_graph_hash: None,
                git_commit: None,
                git_branch: None,
                environment: "cli".into(),
                metadata: HashMap::new(),
            },
            events,
        );

        // 10. Optional audit (best-effort)
        if let Some(ref audit) = self.audit_service
            && self.config.audit_enabled
        {
            let aref: Vec<crate::audit::domain::ExecutionEventRef> = record
                .events
                .iter()
                .map(|e| crate::audit::domain::ExecutionEventRef {
                    event_type: e.event_type.clone(),
                    summary: e.summary.clone(),
                    occurred_at: e.occurred_at,
                    correlation_id: e.correlation_id,
                    status: crate::audit::domain::EventStatus::Success,
                })
                .collect();
            let _ = audit
                .build_and_send(audit_app::BuildEnvelopeInput {
                    execution_id: record.execution_id,
                    template_id: record.planning.template_id.clone(),
                    planning_prompt: record.planning.prompt_hash.clone(),
                    events: aref,
                    metadata: None,
                    sign: false,
                })
                .await;
        }

        // Update state
        if let Some(ref mut s) = *self.current_execution.write().await {
            s.status = final_status;
        }

        tracing::info!(%execution_id, status = ?final_status, "Orchestrator run completed");
        Ok(RunOutput {
            execution_id,
            record,
        })
    }

    async fn plan_only(&self, input: PlanOnlyInput) -> Result<PlanOnlyOutput, OrchestratorError> {
        let result = self
            .planning_pipeline
            .plan_with_graph(planning_dto::PlanWithGraphInput {
                intent: crate::planning::domain::intent::UserIntent::new(input.intent, None),
                execution_id: None,
                enable_generator_fallback: true,
                skip_validation: false,
                repo_root: input.repo_root.clone(),
                module_deps: None,
            })
            .await
            .map_err(|e| OrchestratorError::PlanningFailed {
                detail: e.to_string(),
                intent: String::new(),
            })?;
        Ok(PlanOnlyOutput {
            plan: serde_json::to_value(&result.planning_result).unwrap_or_default(),
            graph: serde_json::to_value(&result.graph).unwrap_or_default(),
        })
    }

    async fn cancel(&self, input: CancelInput) -> Result<CancelOutput, OrchestratorError> {
        let cancel_result = self
            .cancellation_service
            .request_graceful_shutdown(cancel_app::CancelExecutionInput {
                execution_id: input.execution_id.to_string(),
                reason: input.reason.clone(),
                source: "user".into(),
            })
            .await
            .map_err(|e| OrchestratorError::CancellationFailed {
                detail: e.to_string(),
            })?;

        let nodes_cancelled = self
            .execution_service
            .abort_execution(exec_dto::AbortExecutionInput {
                dag_id: input.execution_id,
                reason: input.reason.clone().unwrap_or_default(),
            })
            .await
            .map(|o| o.skipped_count)
            .unwrap_or(0);

        if let Some(ref mut s) = *self.current_execution.write().await {
            s.status = ExecutionStatus::Cancelled;
        }

        self.state_manager
            .save_state(Self::make_final_state(
                input.execution_id,
                ExecutionStatus::Cancelled,
            ))
            .await
            .map_err(|e| OrchestratorError::StatePersistenceFailed {
                detail: e.to_string(),
                state: "Cancelled".into(),
            })?;

        Ok(CancelOutput {
            execution_id: input.execution_id,
            aborted: cancel_result.accepted,
            nodes_cancelled,
        })
    }

    async fn status(&self) -> Result<StatusOutput, OrchestratorError> {
        match &*self.current_execution.read().await {
            Some(s) => Ok(StatusOutput {
                execution_id: s.execution_id,
                status: s.status,
                nodes: s.nodes.clone(),
            }),
            None => Ok(StatusOutput {
                execution_id: Uuid::new_v4(),
                status: ExecutionStatus::Completed,
                nodes: vec![],
            }),
        }
    }

    fn event_bus(&self) -> &dyn event_app::EventBusService {
        &*self.event_bus
    }
}

// ---------------------------------------------------------------------------
// Mocks
// ---------------------------------------------------------------------------

#[cfg(test)]
pub(crate) mod mocks {
    use crate::budget_tracking::application as budget_app;
    use crate::cancellation::application as cancel_app;
    use crate::event_system::application as event_app;
    use crate::execution_engine::application::{dto as exec_dto, service as exec_svc};
    use crate::planning::application::dto as planning_dto;
    use crate::state_persistence::application::{dto as state_dto, service as state_svc};
    use async_trait::async_trait;
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
    use uuid::Uuid;

    fn mock_pr(eid: Uuid) -> crate::planning::domain::result::PlanningResult {
        crate::planning::domain::result::PlanningResult::new(
            eid,
            "mock-template".into(),
            0.95,
            HashMap::new(),
            crate::planning::domain::result::PlanningHash("a".repeat(64)),
            false,
            2,
            500,
            None,
        )
    }

    // --- MockPlanningService ---
    pub(crate) struct MockPlanningService {
        eid: Uuid,
    }
    impl MockPlanningService {
        pub fn new() -> Self {
            Self {
                eid: Uuid::new_v4(),
            }
        }
    }

    #[async_trait]
    impl crate::planning::application::PlanningPipelineService for MockPlanningService {
        async fn plan(
            &self,
            _: planning_dto::PlanInput,
        ) -> Result<planning_dto::PlanOutput, crate::planning::domain::PlanningError> {
            Ok(planning_dto::PlanOutput {
                planning_result: mock_pr(self.eid),
                from_generator: false,
                clarification_used: false,
                total_llm_calls: 2,
                total_llm_tokens: 500,
                completed_at: chrono::Utc::now(),
            })
        }
        async fn plan_with_graph(
            &self,
            _: planning_dto::PlanWithGraphInput,
        ) -> Result<planning_dto::PlanWithGraphOutput, crate::planning::domain::PlanningError>
        {
            Ok(planning_dto::PlanWithGraphOutput {
                planning_result: mock_pr(self.eid),
                graph: crate::dag_engine::domain::TaskGraph::new(),
                node_count: 0,
                validation_passed: true,
                validation_warnings: vec![],
                from_generator: false,
                clarification_used: false,
                total_llm_calls: 2,
                total_llm_tokens: 500,
                completed_at: chrono::Utc::now(),
            })
        }
        async fn check_budget(
            &self,
            _: planning_dto::CheckBudgetInput,
        ) -> Result<planning_dto::CheckBudgetOutput, crate::planning::domain::PlanningError>
        {
            Ok(planning_dto::CheckBudgetOutput {
                has_capacity: true,
                remaining_calls: 100,
                remaining_tokens: 10000,
                max_calls: 1000,
                max_tokens: 100000,
                will_exhaust: false,
            })
        }
        async fn classify_intent(
            &self,
            _: crate::planning::domain::intent::UserIntent,
        ) -> Result<
            crate::planning::domain::classification::ClassificationResult,
            crate::planning::domain::PlanningError,
        > {
            Ok(
                crate::planning::domain::classification::ClassificationResult {
                    alternatives: vec![],
                    requires_clarification: false,
                    needs_generator: false,
                    reasoning: "mock".into(),
                    llm_calls_used: 0,
                    llm_tokens_used: 0,
                },
            )
        }
        async fn extract_parameters(
            &self,
            _: planning_dto::ExtractParametersInput,
        ) -> Result<planning_dto::ExtractParametersOutput, crate::planning::domain::PlanningError>
        {
            Ok(planning_dto::ExtractParametersOutput {
                template_id: "mock".into(),
                parameters: HashMap::new(),
                extra_parameters: HashMap::new(),
                complete: true,
                missing_parameters: vec![],
                llm_calls_used: 0,
                llm_tokens_used: 0,
            })
        }
        async fn generate_graph(
            &self,
            _: planning_dto::GenerateGraphInput,
        ) -> Result<planning_dto::GenerateGraphOutput, crate::planning::domain::PlanningError>
        {
            Ok(planning_dto::GenerateGraphOutput {
                graph: crate::dag_engine::domain::TaskGraph::new(),
                node_count: 0,
                sealed: true,
                from_generator: false,
            })
        }
        async fn validate_plan(
            &self,
            _: planning_dto::ValidatePlanInput,
        ) -> Result<planning_dto::ValidatePlanOutput, crate::planning::domain::PlanningError>
        {
            Ok(planning_dto::ValidatePlanOutput {
                passed: true,
                errors: vec![],
                warnings: vec![],
                checks_performed: 0,
            })
        }
        async fn request_clarification(
            &self,
            _: planning_dto::RequestClarificationInput,
        ) -> Result<planning_dto::RequestClarificationOutput, crate::planning::domain::PlanningError>
        {
            Ok(planning_dto::RequestClarificationOutput {
                question: "?".into(),
                ambiguous_templates: vec![],
                suggested_answers: vec![],
            })
        }
        async fn available_templates(
            &self,
        ) -> Result<planning_dto::AvailableTemplatesOutput, crate::planning::domain::PlanningError>
        {
            Ok(planning_dto::AvailableTemplatesOutput {
                templates: vec![],
                total_count: 0,
            })
        }
        fn execution_id(&self) -> Uuid {
            self.eid
        }
    }

    // --- MockExecutionService ---
    pub(crate) struct MockExecutionService;
    #[async_trait]
    impl exec_svc::ParallelExecutionService for MockExecutionService {
        async fn execute_graph(
            &self,
            _: exec_dto::ExecuteGraphInput,
        ) -> Result<exec_dto::ExecuteGraphOutput, crate::execution_engine::domain::ExecutionError>
        {
            Ok(exec_dto::ExecuteGraphOutput {
                result: crate::execution_engine::domain::ExecutionResult::new(Uuid::new_v4()),
                completed_at: chrono::Utc::now(),
            })
        }
        async fn execute_node(
            &self,
            _: exec_dto::ExecuteNodeInput,
        ) -> Result<exec_dto::ExecuteNodeOutput, crate::execution_engine::domain::ExecutionError>
        {
            unimplemented!()
        }
        async fn get_execution_state(
            &self,
            _: exec_dto::GetExecutionStateInput,
        ) -> Result<
            exec_dto::GetExecutionStateOutput,
            crate::execution_engine::domain::ExecutionError,
        > {
            unimplemented!()
        }
        async fn pause_execution(
            &self,
            _: exec_dto::PauseExecutionInput,
        ) -> Result<exec_dto::PauseExecutionOutput, crate::execution_engine::domain::ExecutionError>
        {
            unimplemented!()
        }
        async fn resume_execution(
            &self,
            _: exec_dto::ResumeExecutionInput,
        ) -> Result<exec_dto::ResumeExecutionOutput, crate::execution_engine::domain::ExecutionError>
        {
            unimplemented!()
        }
        async fn abort_execution(
            &self,
            _: exec_dto::AbortExecutionInput,
        ) -> Result<exec_dto::AbortExecutionOutput, crate::execution_engine::domain::ExecutionError>
        {
            Ok(exec_dto::AbortExecutionOutput {
                dag_id: Uuid::new_v4(),
                completed_count: 0,
                skipped_count: 0,
                aborted_at: chrono::Utc::now(),
            })
        }
        fn on_progress(
            &self,
            _: Box<
                dyn Fn(crate::execution_engine::application::service::ExecutionProgress)
                    + Send
                    + Sync,
            >,
        ) {
        }
    }

    // --- MockStateService ---
    pub(crate) struct MockStateService {
        _saved: AtomicBool,
    }
    impl MockStateService {
        pub fn new() -> Self {
            Self {
                _saved: AtomicBool::new(false),
            }
        }
    }
    #[async_trait]
    impl state_svc::StateManagerService for MockStateService {
        async fn save_state(
            &self,
            _: state_dto::SaveStateInput,
        ) -> Result<state_dto::SaveStateOutput, crate::state_persistence::domain::StateError>
        {
            Ok(state_dto::SaveStateOutput {
                execution_id: Uuid::new_v4(),
                status: crate::state_persistence::domain::ExecutionStatus::Pending,
                node_count: 0,
                saved_at: chrono::Utc::now(),
            })
        }
        async fn load_state(
            &self,
            _: state_dto::LoadStateInput,
        ) -> Result<state_dto::LoadStateOutput, crate::state_persistence::domain::StateError>
        {
            unimplemented!()
        }
        async fn update_node_state(
            &self,
            _: state_dto::NodeStateChangedInput,
        ) -> Result<state_dto::NodeStateChangedOutput, crate::state_persistence::domain::StateError>
        {
            unimplemented!()
        }
        async fn list_executions(
            &self,
            _: state_dto::ListExecutionsInput,
        ) -> Result<state_dto::ListExecutionsOutput, crate::state_persistence::domain::StateError>
        {
            unimplemented!()
        }
        async fn delete_state(
            &self,
            _: Uuid,
        ) -> Result<(), crate::state_persistence::domain::StateError> {
            Ok(())
        }
    }

    // --- MockCancellationService ---
    pub(crate) struct MockCancellationService;
    #[async_trait]
    impl cancel_app::CancellationService for MockCancellationService {
        async fn request_graceful_shutdown(
            &self,
            _: cancel_app::CancelExecutionInput,
        ) -> Result<cancel_app::CancelExecutionOutput, crate::cancellation::domain::CancellationError>
        {
            Ok(cancel_app::CancelExecutionOutput {
                accepted: true,
                signal: crate::cancellation::domain::ShutdownSignal::Graceful,
                affected_tasks: 0,
                was_already_cancelling: false,
            })
        }
        async fn request_immediate_abort(
            &self,
            _: cancel_app::CancelExecutionInput,
        ) -> Result<cancel_app::CancelExecutionOutput, crate::cancellation::domain::CancellationError>
        {
            unimplemented!()
        }
        async fn await_shutdown(
            &self,
            _: cancel_app::ShutdownInput,
        ) -> Result<cancel_app::ShutdownOutput, crate::cancellation::domain::CancellationError>
        {
            unimplemented!()
        }
        fn is_cancelled(&self) -> bool {
            false
        }
        fn current_signal(&self) -> Option<crate::cancellation::domain::ShutdownSignal> {
            None
        }
        async fn status(&self) -> cancel_app::ShutdownStatusOutput {
            cancel_app::ShutdownStatusOutput {
                is_cancelled: false,
                current_signal: Some(crate::cancellation::domain::ShutdownSignal::Graceful),
                running_tasks: 0,
                completed_tasks: 0,
                cancelled_tasks: 0,
                shutdown_complete: false,
                elapsed_since_request_ms: Some(0),
            }
        }
        fn subscribe(
            &self,
        ) -> tokio::sync::watch::Receiver<crate::cancellation::domain::ShutdownSignal> {
            let (tx, rx) =
                tokio::sync::watch::channel(crate::cancellation::domain::ShutdownSignal::Graceful);
            let _ = tx;
            rx
        }
        fn cancellation_token(&self) -> tokio_util::sync::CancellationToken {
            tokio_util::sync::CancellationToken::new()
        }
    }

    // --- MockEventBusService ---
    pub(crate) struct MockEventBusService {
        count: AtomicU32,
        sender: tokio::sync::broadcast::Sender<crate::event_system::domain::ExecutionEvent>,
    }
    impl MockEventBusService {
        pub fn new() -> Self {
            let (sender, _) = tokio::sync::broadcast::channel(16);
            Self {
                count: AtomicU32::new(0),
                sender,
            }
        }
    }
    #[async_trait]
    impl event_app::EventBusService for MockEventBusService {
        async fn publish(
            &self,
            _: event_app::PublishEventInput,
        ) -> Result<event_app::PublishEventOutput, crate::event_system::domain::EventSystemError>
        {
            self.count.fetch_add(1, Ordering::SeqCst);
            Ok(event_app::PublishEventOutput {
                sequence: self.count.load(Ordering::SeqCst) as u64,
                subscriber_count: 0,
                had_laggers: false,
            })
        }
        async fn subscribe(
            &self,
            _: event_app::SubscribeInput,
        ) -> Result<event_app::SubscribeOutput, crate::event_system::domain::EventSystemError>
        {
            Ok(event_app::SubscribeOutput {
                success: true,
                subscriber_name: Uuid::new_v4().to_string(),
                active_subscriber_count: 0,
            })
        }
        async fn drain_persisted(
            &self,
            _: event_app::DrainPersistedInput,
        ) -> Result<event_app::DrainPersistedOutput, crate::event_system::domain::EventSystemError>
        {
            Ok(event_app::DrainPersistedOutput {
                events: vec![],
                count: 0,
                cleared: true,
            })
        }
        async fn query_events(
            &self,
            _: event_app::QueryEventsInput,
        ) -> Result<event_app::QueryEventsOutput, crate::event_system::domain::EventSystemError>
        {
            Ok(event_app::QueryEventsOutput {
                events: vec![],
                total: 0,
                has_more: false,
            })
        }
        async fn status(
            &self,
            _: event_app::EventBusStatusInput,
        ) -> Result<event_app::EventBusStatus, crate::event_system::domain::EventSystemError>
        {
            Ok(event_app::EventBusStatus {
                current_sequence: 0,
                active_subscriber_count: 0,
                channel_capacity: 10000,
                buffer_capacity: 10000,
                persisted_count: 0,
            })
        }
        async fn event_count(
            &self,
        ) -> Result<event_app::EventCountOutput, crate::event_system::domain::EventSystemError>
        {
            Ok(event_app::EventCountOutput {
                total: 0,
                persisted: 0,
                drained: 0,
            })
        }
        fn subscribe_receiver(
            &self,
        ) -> tokio::sync::broadcast::Receiver<crate::event_system::domain::ExecutionEvent> {
            self.sender.subscribe()
        }
    }

    // --- MockAuditService ---
    pub(crate) struct MockAuditService;
    impl MockAuditService {
        pub fn new() -> Self {
            Self
        }
    }
    #[async_trait]
    impl crate::audit::application::AuditService for MockAuditService {
        async fn build_and_send(
            &self,
            _: crate::audit::application::BuildEnvelopeInput,
        ) -> Result<crate::audit::application::BuildEnvelopeOutput, crate::audit::domain::AuditError>
        {
            Ok(crate::audit::application::BuildEnvelopeOutput {
                envelope: crate::audit::domain::AuditEnvelope {
                    execution_id: Uuid::new_v4(),
                    timestamp: chrono::Utc::now(),
                    template_id: "mock".into(),
                    planning_hash: "hash".into(),
                    events: vec![],
                    signature: None,
                },
                signed: false,
                event_count: 0,
            })
        }
        async fn retry_pending(
            &self,
        ) -> Result<crate::audit::application::RetryPendingOutput, crate::audit::domain::AuditError>
        {
            Ok(crate::audit::application::RetryPendingOutput {
                delivered: 0,
                still_pending: 0,
                dropped: 0,
            })
        }
        async fn status(
            &self,
        ) -> Result<crate::audit::application::AuditStatusOutput, crate::audit::domain::AuditError>
        {
            Ok(crate::audit::application::AuditStatusOutput {
                pending_count: 0,
                circuit_breaker_state: crate::audit::domain::CircuitBreakerState::Closed,
                backend_available: false,
            })
        }
    }

    // --- MockBudgetService ---
    pub(crate) struct MockBudgetService;
    #[async_trait]
    impl budget_app::LlmBudgetService for MockBudgetService {
        async fn reserve(
            &self,
            _: budget_app::ReserveBudgetInput,
        ) -> Result<budget_app::ReserveBudgetOutput, crate::budget_tracking::domain::LlmBudgetError>
        {
            Ok(budget_app::ReserveBudgetOutput {
                reservation: crate::budget_tracking::domain::LlmBudgetReservationState::new(0, 100),
                remaining_calls: 100,
                remaining_tokens: 10000,
                calls_used: 0,
                tokens_used_before_reservation: 0,
            })
        }
        async fn commit(
            &self,
            _: budget_app::CommitReservationInput,
        ) -> Result<
            budget_app::CommitReservationOutput,
            crate::budget_tracking::domain::LlmBudgetError,
        > {
            Ok(budget_app::CommitReservationOutput {
                remaining_calls: 100,
                remaining_tokens: 10000,
                total_calls_used: 0,
                total_tokens_used: 0,
                reservation: crate::budget_tracking::domain::LlmBudgetReservationState::new(0, 100),
                warnings_triggered: vec![],
            })
        }
        async fn get_status(
            &self,
            _: budget_app::GetBudgetStatusInput,
        ) -> Result<budget_app::GetBudgetStatusOutput, crate::budget_tracking::domain::LlmBudgetError>
        {
            Ok(budget_app::GetBudgetStatusOutput {
                max_calls: 1000,
                max_tokens: 100000,
                calls_used: 0,
                tokens_used: 0,
                remaining_calls: 1000,
                remaining_tokens: 100000,
                call_usage_ratio: 0.0,
                token_usage_ratio: 0.0,
                active_warnings: vec![],
                label: "mock".into(),
            })
        }
        fn has_capacity(&self) -> bool {
            true
        }
        fn active_warnings(&self) -> Vec<budget_app::BudgetWarningInfo> {
            vec![]
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_run_returns_execution_id() {
        let orch = OrchestratorServiceImpl::default_test();
        let out = orch
            .run(RunInput {
                intent: "test".into(),
                config: serde_json::json!({}),
                repo_root: "/tmp/t".into(),
                enforcement_preset: None,
            })
            .await
            .unwrap();
        assert_ne!(out.execution_id, Uuid::nil());
        assert_eq!(out.record.execution_id, out.execution_id);
        assert_eq!(out.record.status, ExecutionStatus::Completed);
    }

    #[tokio::test]
    async fn test_run_planning_metadata() {
        let orch = OrchestratorServiceImpl::default_test();
        let out = orch
            .run(RunInput {
                intent: "test".into(),
                config: serde_json::json!({}),
                repo_root: "/tmp/t".into(),
                enforcement_preset: None,
            })
            .await
            .unwrap();
        assert_eq!(out.record.planning.template_id, "mock-template");
        assert!((out.record.planning.confidence - 0.95).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn test_run_timestamps() {
        let orch = OrchestratorServiceImpl::default_test();
        let out = orch
            .run(RunInput {
                intent: "test".into(),
                config: serde_json::json!({}),
                repo_root: "/tmp/t".into(),
                enforcement_preset: None,
            })
            .await
            .unwrap();
        assert!(out.record.completed_at.is_some());
    }

    #[tokio::test]
    async fn test_run_context() {
        let orch = OrchestratorServiceImpl::default_test();
        let out = orch
            .run(RunInput {
                intent: "test".into(),
                config: serde_json::json!({}),
                repo_root: "/tmp/repo".into(),
                enforcement_preset: None,
            })
            .await
            .unwrap();
        assert_eq!(out.record.context.repo_root, "/tmp/repo");
    }

    #[tokio::test]
    async fn test_plan_only() {
        let orch = OrchestratorServiceImpl::default_test();
        assert!(
            orch.plan_only(PlanOnlyInput {
                intent: "test".into(),
                config: serde_json::json!({}),
                repo_root: "/tmp/t".into(),
            })
            .await
            .is_ok()
        );
    }

    #[tokio::test]
    async fn test_cancel() {
        let orch = OrchestratorServiceImpl::default_test();
        let out = orch
            .cancel(CancelInput {
                execution_id: Uuid::new_v4(),
                reason: Some("test".into()),
            })
            .await
            .unwrap();
        assert!(out.aborted);
    }

    #[tokio::test]
    async fn test_status_after_run() {
        let orch = OrchestratorServiceImpl::default_test();
        let _ = orch
            .run(RunInput {
                intent: "test".into(),
                config: serde_json::json!({}),
                repo_root: "/tmp/t".into(),
                enforcement_preset: None,
            })
            .await
            .unwrap();
        assert_eq!(
            orch.status().await.unwrap().status,
            ExecutionStatus::Completed
        );
    }

    #[tokio::test]
    async fn test_planning_failure() {
        struct FailPlan;
        #[async_trait]
        impl crate::planning::application::PlanningPipelineService for FailPlan {
            async fn plan(
                &self,
                _: planning_dto::PlanInput,
            ) -> Result<planning_dto::PlanOutput, crate::planning::domain::PlanningError>
            {
                Err(crate::planning::domain::PlanningError::NoMatchingTemplate {
                    intent_preview: "test".into(),
                    templates_evaluated: 0,
                })
            }
            async fn plan_with_graph(
                &self,
                _: planning_dto::PlanWithGraphInput,
            ) -> Result<planning_dto::PlanWithGraphOutput, crate::planning::domain::PlanningError>
            {
                Err(crate::planning::domain::PlanningError::NoMatchingTemplate {
                    intent_preview: "test".into(),
                    templates_evaluated: 0,
                })
            }
            async fn check_budget(
                &self,
                _: planning_dto::CheckBudgetInput,
            ) -> Result<planning_dto::CheckBudgetOutput, crate::planning::domain::PlanningError>
            {
                unimplemented!()
            }
            async fn classify_intent(
                &self,
                _: crate::planning::domain::intent::UserIntent,
            ) -> Result<
                crate::planning::domain::classification::ClassificationResult,
                crate::planning::domain::PlanningError,
            > {
                unimplemented!()
            }
            async fn extract_parameters(
                &self,
                _: planning_dto::ExtractParametersInput,
            ) -> Result<planning_dto::ExtractParametersOutput, crate::planning::domain::PlanningError>
            {
                unimplemented!()
            }
            async fn generate_graph(
                &self,
                _: planning_dto::GenerateGraphInput,
            ) -> Result<planning_dto::GenerateGraphOutput, crate::planning::domain::PlanningError>
            {
                unimplemented!()
            }
            async fn validate_plan(
                &self,
                _: planning_dto::ValidatePlanInput,
            ) -> Result<planning_dto::ValidatePlanOutput, crate::planning::domain::PlanningError>
            {
                unimplemented!()
            }
            async fn request_clarification(
                &self,
                _: planning_dto::RequestClarificationInput,
            ) -> Result<
                planning_dto::RequestClarificationOutput,
                crate::planning::domain::PlanningError,
            > {
                unimplemented!()
            }
            async fn available_templates(
                &self,
            ) -> Result<
                planning_dto::AvailableTemplatesOutput,
                crate::planning::domain::PlanningError,
            > {
                unimplemented!()
            }
            fn execution_id(&self) -> Uuid {
                Uuid::new_v4()
            }
        }
        let orch = OrchestratorServiceImpl::new(
            OrchestratorConfig::default(),
            Arc::new(FailPlan),
            Arc::new(mocks::MockExecutionService),
            Arc::new(mocks::MockStateService::new()),
            Arc::new(mocks::MockCancellationService),
            Arc::new(mocks::MockEventBusService::new()),
            None,
            Arc::new(mocks::MockBudgetService),
            None,
        );
        let e = orch
            .run(RunInput {
                intent: "test".into(),
                config: serde_json::json!({}),
                repo_root: "/tmp/t".into(),
                enforcement_preset: None,
            })
            .await
            .unwrap_err();
        match e {
            OrchestratorError::PlanningFailed { detail, intent } => {
                assert!(detail.contains("No matching template"));
                assert_eq!(intent, "test");
            }
            _ => panic!("expected PlanningFailed"),
        }
    }
}
