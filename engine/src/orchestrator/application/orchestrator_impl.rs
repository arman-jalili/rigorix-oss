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
use crate::plan_validation::application::dto::ValidateInput;
use crate::plan_validation::application::service::ValidationLoopService;
use crate::plan_validation::domain::loop_config::ValidationLoopConfig;
use crate::planning::application::dto as planning_dto;
use crate::policy_engine::application::dto::EvaluatePolicyInput;
use crate::policy_engine::application::engine::PolicyEngineService;
use crate::policy_engine::domain::{DiffScope, LaneBlocker, LaneContext, ReviewStatus};
use crate::quality_gates::application::dto::{ClassifyTestScopeInput, EvaluateGateInput};
use crate::quality_gates::application::service::QualityGateService;
use crate::state_persistence::application::{dto as state_dto, service as state_svc};

pub struct OrchestratorServiceImpl {
    config: OrchestratorConfig,
    planning_pipeline: Arc<dyn crate::planning::application::PlanningPipelineService>,
    execution_service: Arc<dyn exec_svc::ParallelExecutionService>,
    state_manager: Arc<dyn state_svc::StateManagerService>,
    cancellation_service: Arc<dyn cancel_app::CancellationService>,
    event_bus: Arc<dyn event_app::EventBusService>,
    audit_service: Option<Arc<dyn audit_app::AuditService>>,
    budget_service: Arc<dyn budget_app::LlmBudgetService>,
    code_graph_service: Option<Arc<dyn CodeGraphServiceTrait>>,
    quality_gate_service: Option<Arc<dyn QualityGateService>>,
    policy_engine: Option<Arc<dyn PolicyEngineService>>,
    validation_loop_service: Option<Arc<dyn ValidationLoopService>>,
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
            quality_gate_service: None,
            policy_engine: None,
            validation_loop_service: None,
            current_execution: Arc::new(RwLock::new(None)),
        }
    }

    /// Set the validation loop service for self-correcting plan→execute→verify cycles.
    pub fn with_validation_loop(mut self, svc: Arc<dyn ValidationLoopService>) -> Self {
        self.validation_loop_service = Some(svc);
        self
    }

    /// Set the quality gate service for post-execution quality evaluation.
    pub fn with_quality_gate_service(mut self, svc: Arc<dyn QualityGateService>) -> Self {
        self.quality_gate_service = Some(svc);
        self
    }

    /// Set the policy engine for post-execution policy evaluation.
    pub fn with_policy_engine(mut self, engine: Arc<dyn PolicyEngineService>) -> Self {
        self.policy_engine = Some(engine);
        self
    }

    #[cfg(test)]
    pub fn default_test() -> Self {
        Self::new(
            OrchestratorConfig::default(),
            Arc::new(super::super::orchestrator_mocks::MockPlanningService::new()),
            Arc::new(super::super::orchestrator_mocks::MockExecutionService),
            Arc::new(super::super::orchestrator_mocks::MockStateService::new()),
            Arc::new(super::super::orchestrator_mocks::MockCancellationService),
            Arc::new(super::super::orchestrator_mocks::MockEventBusService::new()),
            None,
            Arc::new(super::super::orchestrator_mocks::MockBudgetService),
            None,
        )
    }

    fn gen_id(&self) -> Uuid {
        Uuid::now_v7()
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

        // ── Validation loop (if enabled, wraps plan→execute→verify) ──
        if let Some(ref validation_svc) = self.validation_loop_service {
            let config = ValidationLoopConfig {
                max_iterations: 3,
                max_cumulative_tokens: 50000,
                ..ValidationLoopConfig::default()
            };
            let validate_input = ValidateInput {
                intent: crate::planning::domain::intent::UserIntent::new(
                    input.intent.clone(),
                    Some(execution_id),
                ),
                execution_id: Some(execution_id),
                config,
                existing_template: None,
            };
            let outcome = validation_svc.validate(validate_input).await.map_err(|e| {
                OrchestratorError::ExecutionFailed {
                    detail: format!("Validation loop error: {e}"),
                    nodes_completed: 0,
                    nodes_remaining: 0,
                }
            })?;

            let record = ExecutionRecord::new(execution_id, started_at);

            tracing::info!(%execution_id, iterations = outcome.iterations, "Validation loop completed");
            return Ok(RunOutput {
                execution_id,
                record,
            });
        }

        // ── Legacy path (no validation loop) ──

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

        // 3. Check budget before planning (LLM calls are expensive)
        if !self.budget_service.has_capacity() {
            return Err(OrchestratorError::ExecutionFailed {
                detail: "Budget exhausted before planning phase".to_string(),
                nodes_completed: 0,
                nodes_remaining: 0,
            });
        }

        // 4. Run planning pipeline
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

        // 7a. Quality Gate evaluation
        if let Some(ref quality_svc) = self.quality_gate_service {
            let classify_input = ClassifyTestScopeInput {
                targeted_tests_run: true,
                package_tests_run: true,
                workspace_tests_run: final_status != ExecutionStatus::Failed,
                lint_passed: false,
                format_passed: false,
                audit_passed: false,
            };
            if let Ok(classify_out) = quality_svc.classify_test_scope(classify_input).await {
                use crate::quality_gates::domain::GreenContract;
                let eval_input = EvaluateGateInput {
                    contract: GreenContract::default(),
                    observed_level: Some(classify_out.level),
                    task_id: Some(execution_id.to_string()),
                };
                if let Ok(eval_out) = quality_svc.evaluate_gate(eval_input).await {
                    tracing::info!(
                        execution_id = %execution_id,
                        quality = %eval_out.summary,
                        "Quality gate evaluated"
                    );
                }
            }
        }

        // 7b. Policy Engine evaluation
        if let Some(ref policy_svc) = self.policy_engine {
            let green_level = if final_status == ExecutionStatus::Completed {
                3u8
            } else if final_status == ExecutionStatus::PartialFailure {
                1u8
            } else {
                0u8
            };

            let context = LaneContext {
                lane_id: execution_id.to_string(),
                green_level,
                branch_freshness_secs: 0,
                blocker: LaneBlocker::None,
                review_status: ReviewStatus::Pending,
                diff_scope: DiffScope::Scoped,
                completed: final_status == ExecutionStatus::Completed,
                reconciled: false,
            };

            let eval_policy_input = EvaluatePolicyInput {
                context,
                rule_filter: None,
            };
            if let Ok(eval_policy_out) = policy_svc.evaluate(eval_policy_input).await {
                for action in eval_policy_out.actions {
                    tracing::info!(
                        execution_id = %execution_id,
                        action = ?action,
                        "Policy action dispatched"
                    );
                }
            }
        }

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

// Mocks moved to orchestrator_mocks.rs
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
            Arc::new(super::super::orchestrator_mocks::MockExecutionService),
            Arc::new(super::super::orchestrator_mocks::MockStateService::new()),
            Arc::new(super::super::orchestrator_mocks::MockCancellationService),
            Arc::new(super::super::orchestrator_mocks::MockEventBusService::new()),
            None,
            Arc::new(super::super::orchestrator_mocks::MockBudgetService),
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
