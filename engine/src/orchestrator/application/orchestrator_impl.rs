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
    ApproveExecutionInput, ApproveExecutionOutput, CancelInput, CancelOutput, NodeState,
    PlanFromTemplateInput, PlanOnlyInput, PlanOnlyOutput, RunFromTemplateInput, RunInput,
    RunOutput, StatusOutput,
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
use crate::identity::domain::IdentityRef;
use crate::plan_validation::application::dto::ValidateInput;
use crate::plan_validation::application::service::ValidationLoopService;
use crate::plan_validation::domain::loop_config::ValidationLoopConfig;
use crate::planning::application::dto as planning_dto;
use crate::policy_engine::application::dto::EvaluatePolicyInput;
use crate::policy_engine::application::engine::PolicyEngineService;
use crate::policy_engine::domain::{DiffScope, LaneBlocker, LaneContext, ReviewStatus};
use crate::quality_gates::application::dto::{ClassifyTestScopeInput, EvaluateGateInput};
use crate::quality_gates::application::service::QualityGateService;
use crate::scored_evaluation::application::ScoredEvaluationService;
use crate::scored_evaluation::application::dto::EvaluateInput as ScoredEvalInput;
use crate::scored_evaluation::domain::Rubric;
use crate::sequence_policy::application::dto::PlannedStep;
use crate::sequence_policy::application::service::SequencePolicyService;
use crate::state_persistence::application::{dto as state_dto, service as state_svc};

mod effect_key;
mod helpers;
mod policy_pipeline;

mod approval_flow;

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
    scored_evaluation_service: Option<Arc<dyn ScoredEvaluationService>>,
    policy_engine: Option<Arc<dyn PolicyEngineService>>,
    validation_loop_service: Option<Arc<dyn ValidationLoopService>>,
    sequence_policy_service: Option<Arc<dyn SequencePolicyService>>,
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
            scored_evaluation_service: None,
            policy_engine: None,
            validation_loop_service: None,
            sequence_policy_service: None,
            current_execution: Arc::new(RwLock::new(None)),
        }
    }

    /// Set the validation loop service for self-correcting plan→execute→verify cycles.
    pub fn with_validation_loop(mut self, svc: Arc<dyn ValidationLoopService>) -> Self {
        self.validation_loop_service = Some(svc);
        self
    }

    /// Set the audit service for sending execution audit envelopes.
    pub fn with_audit_service(mut self, audit: Arc<dyn audit_app::AuditService>) -> Self {
        self.audit_service = Some(audit);
        self
    }

    /// Set the quality gate service for post-execution quality evaluation.
    pub fn with_quality_gate_service(mut self, svc: Arc<dyn QualityGateService>) -> Self {
        self.quality_gate_service = Some(svc);
        self
    }

    /// Set the scored evaluation service for artifact quality scoring.
    pub fn with_scored_evaluation_service(mut self, svc: Arc<dyn ScoredEvaluationService>) -> Self {
        self.scored_evaluation_service = Some(svc);
        self
    }

    /// Set the policy engine for post-execution policy evaluation.
    pub fn with_policy_engine(mut self, engine: Arc<dyn PolicyEngineService>) -> Self {
        self.policy_engine = Some(engine);
        self
    }

    /// Set the sequence-policy service for R2 plan-time evaluation of ordered
    /// runbooks (`run_from_template` / `plan_from_template`).
    ///
    /// Evaluation runs **before** the DAG graph is sealed: a `promote` match
    /// flags the later matched step `requires_approval = true` (the existing
    /// approval pause/resume chain decides); a `deny` match refuses the plan.
    /// When unset (default) the runbook executes unchanged — no gating.
    pub fn with_sequence_policy(mut self, svc: Arc<dyn SequencePolicyService>) -> Self {
        self.sequence_policy_service = Some(svc);
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

        let pmeta = Self::planning_meta(
            &plan_out.planning_result,
            Some(&plan_out.graph),
            &self.config.model_version,
        );

        // GAP-A-11: enforce the action-level LLM budget (--max-llm-calls /
        // --max-llm-tokens) against the planning metadata.
        if let Some(limit) = self.config.max_llm_calls
            && pmeta.llm_calls > limit
        {
            return Err(OrchestratorError::PlanningFailed {
                detail: format!(
                    "LLM call budget exceeded: {} calls > limit {}",
                    pmeta.llm_calls, limit
                ),
                intent: input.intent.clone(),
            });
        }
        if let Some(limit) = self.config.max_llm_tokens
            && pmeta.total_tokens as u64 > limit
        {
            return Err(OrchestratorError::PlanningFailed {
                detail: format!(
                    "LLM token budget exceeded: {} tokens > limit {}",
                    pmeta.total_tokens, limit
                ),
                intent: input.intent.clone(),
            });
        }

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

        // 5a. Post-execution scored evaluation — only nodes with ScoredEvaluation validation
        if let Some(ref se_svc) = self.scored_evaluation_service {
            for tr in &task_results {
                if tr.status != TaskStatus::Success {
                    continue;
                }
                if !plan_out.scored_node_ids.contains(&tr.node_id) {
                    continue;
                }
                let node_id = match uuid::Uuid::parse_str(&tr.node_id) {
                    Ok(id) => id,
                    Err(_) => continue,
                };
                let artifact = tr.output.clone().unwrap_or_default();
                let rubric = Rubric::inline(serde_json::json!({
                    "scoring_key": "default"
                }));
                let input = ScoredEvalInput::new(
                    serde_json::Value::String(artifact),
                    rubric,
                    execution_id,
                    node_id,
                    &tr.node_name,
                );
                match se_svc.evaluate(input).await {
                    Ok(output) => {
                        tracing::debug!(
                            node = %tr.node_name,
                            passed = output.result.passed,
                            "Scored evaluation completed"
                        );
                    }
                    Err(e) => {
                        tracing::warn!(
                            node = %tr.node_name,
                            error = %e,
                            "Scored evaluation skipped"
                        );
                    }
                }
            }
        }

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

        // 7. Save final state (legacy single-flow path — no parallel
        // execution-state map in scope, so None for exec_node_states)
        self.state_manager
            .save_state(Self::make_final_state(execution_id, final_status, None))
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
                scoring_scores: std::collections::HashMap::new(),
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
                            crate::event_system::domain::ExecutionEvent::AuditEnvelopeDelivered {
                                timestamp,
                                ..
                            } => *timestamp,
                            crate::event_system::domain::ExecutionEvent::AuditEnvelopeQueued {
                                timestamp,
                                ..
                            } => *timestamp,
                            crate::event_system::domain::ExecutionEvent::AuditEnvelopeDropped {
                                timestamp,
                                ..
                            } => *timestamp,
                            crate::event_system::domain::ExecutionEvent::CircuitBreakerStateChanged {
                                timestamp,
                                ..
                            } => *timestamp,
                            crate::event_system::domain::ExecutionEvent::AuditEnvelopeCreated {
                                timestamp,
                                ..
                            } => *timestamp,
                            | crate::event_system::domain::ExecutionEvent::ApprovalRecorded {
                                timestamp,
                                ..
                            }
                            | crate::event_system::domain::ExecutionEvent::IntentMismatchDetected {
                                timestamp,
                                ..
                            }
                            | crate::event_system::domain::ExecutionEvent::ScopeViolationRecorded {
                                timestamp,
                                ..
                            }
                            | crate::event_system::domain::ExecutionEvent::SequenceRuleMatched {
                                timestamp,
                                ..
                            }
                            | crate::event_system::domain::ExecutionEvent::SequencePolicyDenied {
                                timestamp,
                                ..
                            }
                            | crate::event_system::domain::ExecutionEvent::SequencePolicyConfigError {
                                timestamp,
                                ..
                            } => *timestamp,
                            | crate::event_system::domain::ExecutionEvent::RequirementUnmet {
                                timestamp,
                                ..
                            }
                            | crate::event_system::domain::ExecutionEvent::RequirementPromoted {
                                timestamp,
                                ..
                            } => *timestamp,
                        };
                        ExecutionEventInfo {
                            event_type: pe.event.event_type_name().to_string(),
                            summary: pe.event.event_type_name().to_string(),
                            occurred_at: ts,
                            correlation_id: Some(*pe.event.execution_id()),
                            payload: pe.event.payload_json(),
                            status: pe.event.event_info_status(),
                        }
                    })
                    .collect()
            })
            .unwrap_or_default();

        // 9. Build record
        let (git_commit, git_branch) = Self::detect_git_info(&input.repo_root);
        let record = self.build_record(
            execution_id,
            started_at,
            final_status,
            Some(pmeta),
            task_results,
            ExecutionContext {
                repo_root: input.repo_root,
                symbol_graph_hash: None,
                git_commit,
                git_branch,
                environment: if std::env::var("GITHUB_ACTIONS").as_deref() == Ok("true") {
                    "rigorix_action".into()
                } else {
                    "rigorix_cli".into()
                },
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
                .map(|e| {
                    let audit_status = match e.status {
                        EventInfoStatus::Success => crate::audit::domain::EventStatus::Success,
                        EventInfoStatus::Failure => crate::audit::domain::EventStatus::Failure,
                        EventInfoStatus::Info => crate::audit::domain::EventStatus::Success,
                    };
                    crate::audit::domain::ExecutionEventRef {
                        event_type: e.event_type.clone(),
                        summary: e.summary.clone(),
                        occurred_at: e.occurred_at,
                        correlation_id: e.correlation_id,
                        status: audit_status,
                        payload: e.payload.clone(),
                    }
                })
                .collect();
            let _ = audit
                .build_and_send(audit_app::BuildEnvelopeInput {
                    execution_id: record.execution_id,
                    template_id: record.planning.template_id.clone(),
                    planning_prompt: record.planning.prompt_hash.clone(),
                    events: aref,
                    source: Some(record.context.environment.clone()),
                    total_tokens: record.planning.total_tokens,
                    duration_ms: record.duration_ms,
                    git_commit: record.context.git_commit.clone(),
                    git_branch: record.context.git_branch.clone(),
                    model_version: record.planning.model_version.clone(),
                    planning_prompt_content: self.planning_prompt_content(&record.planning),
                    file_paths: Self::extract_file_paths(&record.task_results),
                    metadata: None,
                    scoring_results: self.collect_scoring_results(record.execution_id).await,
                    sign: true,
                    repository: input.repository.clone(),
                    author: input.author.clone(),
                    identity: input.identity.as_ref().map(IdentityRef::from_claim),
                    effect_key: effect_key::run_effect_key(&record.planning.parameters),
                    // ADR-016: chain link + history-policy opt-in are resolved
                    // by `AuditServiceImpl::build_and_send` before signing.
                    producer_id: None,
                    sequence: None,
                    prev_hash: None,
                    history_policy: None,
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
            sequence_findings: Vec::new(),
            requirement_findings: Vec::new(),
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
                None,
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

    // ── From-template methods (skip intent→plan pipeline) ────────────

    async fn run_from_template(
        &self,
        input: RunFromTemplateInput,
    ) -> Result<RunOutput, OrchestratorError> {
        let execution_id = input.execution_id.unwrap_or_else(|| self.gen_id());
        let started_at = chrono::Utc::now();
        tracing::info!(%execution_id, template=%input.template_name, "run_from_template");

        // L1 identity gate (F-20260907-05) — run BEFORE the sequence-policy
        // gate so the error names the missing identity rather than a
        // possibly-confusing sequence denial. Steps declaring
        // `require_identity = true` refuse an unauthenticated / unverified
        // caller at plan time; the step's tool is never called.
        self.enforce_identity_requirement(&input.steps, input.identity.as_ref())?;

        // Sequence-policy (R2) plan-time gate — evaluate the ordered runbook
        // BEFORE any state is written or step executes. Promote matches flip
        // the later step to `requires_approval = true` (reusing the approval
        // pause/resume chain below); deny matches refuse the whole runbook
        // fail-closed — the forbidden sequence never executes and the denied
        // step's tool is never called. An evaluation error refuses the plan
        // too (fail closed on corrupt/over-cap rule config).
        // L2 (F-20260907-05): the policy principal is the ATTESTED claim's
        // subject when present (caller-supplied `author` stays display-only
        // for policy purposes).
        let principal = input
            .identity
            .as_ref()
            .map(|i| i.subject.as_str())
            .or(input.author.as_deref());
        // R2 (sequence policy) → R9 (operator requirements), composed and
        // fail-closed (see `policy_pipeline`) — replaces the inline gate
        // arms. Requirements evaluate the sequence-enforced step list.
        let (gated_steps, _findings, _requirement_findings) =
            policy_pipeline::PolicyPipeline::new(self)
                .apply(
                    &input.steps,
                    Some(execution_id),
                    principal,
                    input.identity.as_ref(),
                )
                .await?;

        // Init current execution state
        *self.current_execution.write().await = Some(CurrentExecutionState {
            execution_id,
            status: ExecutionStatus::Failed,
            nodes: vec![],
            started_at,
        });

        // 1. Build DAG directly from pre-resolved steps (enforced steps when
        // a promote rule/requirement matched — the later step is built
        // approval-gated).
        let steps: &[super::dto::TemplateStepDef] = &gated_steps;
        let graph = self.build_graph_from_steps(steps)?;
        let node_order: Vec<String> = graph.nodes().map(|n| n.name.clone()).collect();

        let pmeta = PlanningMetadata {
            template_id: input.template_name.clone(),
            confidence: 1.0,
            llm_calls: 0,
            total_tokens: 0,
            prompt_hash: String::new(),
            parameters: input
                .steps
                .iter()
                .enumerate()
                .map(|(i, s)| {
                    (
                        format!("step_{}", i),
                        serde_json::to_string(&s.parameters).unwrap_or_default(),
                    )
                })
                .collect(),
            generated_toml: None,
            node_order,
            model_version: self.config.model_version.clone(),
        };

        // 2. Save initial state
        self.state_manager
            .save_state(Self::make_pending_state(execution_id))
            .await
            .map_err(|e| OrchestratorError::StatePersistenceFailed {
                detail: e.to_string(),
                state: "Pending".into(),
            })?;

        // 2a. Budget gate — template runs consume one budget call per step.
        // Reserve the full runbook up front so an exhausted budget refuses
        // the run deterministically (pre-execution) instead of failing
        // halfway through a consequential operation. Reservations are
        // committed after execution (actual = 1 call per step); if the run
        // errors before that, the Drop guard rolls them back.
        let mut reservations: Vec<budget_app::ReserveBudgetOutput> = Vec::new();
        for step in &input.steps {
            match self
                .budget_service
                .reserve(budget_app::ReserveBudgetInput {
                    execution_id,
                    estimated_tokens: 1,
                    call_label: Some(step.name.clone()),
                })
                .await
            {
                Ok(out) => reservations.push(out),
                Err(crate::budget_tracking::domain::LlmBudgetError::MaxCallsExceeded {
                    used,
                    max,
                }) => {
                    return Err(OrchestratorError::Internal {
                        detail: format!(
                            "Budget exhausted: runbook needs {} steps but budget has {} of {} calls used — top up the budget or shrink the runbook",
                            input.steps.len(),
                            used,
                            max
                        ),
                        source_module: "orchestrator".into(),
                    });
                }
                Err(e) => {
                    return Err(OrchestratorError::Internal {
                        detail: format!("Budget reservation failed: {e}"),
                        source_module: "orchestrator".into(),
                    });
                }
            }
        }

        // 3. Execute DAG — keep a clone of the graph so an approval pause can
        // be persisted for cross-process resume (GAP-3).
        let graph_for_resume = graph.clone();
        let exec_output = self
            .execution_service
            .execute_graph(exec_dto::ExecuteGraphInput {
                dag_id: execution_id,
                graph: Some(graph),
                config_override: None,
            })
            .await
            .map_err(|e| OrchestratorError::ExecutionFailed {
                detail: e.to_string(),
                nodes_completed: 0,
                nodes_remaining: 0,
            })?;
        let task_results = exec_output
            .result
            .node_results
            .clone()
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
            .collect::<Vec<_>>();
        let approval_pending = exec_output.approval_pending;
        let pending_approval_steps = exec_output.pending_approval_steps.clone();

        if approval_pending {
            tracing::info!(
                %execution_id,
                pending_steps = ?pending_approval_steps,
                "Execution paused for human approval"
            );
        }

        // 3a. Post-execution scored evaluation — only nodes where step has evaluate_score: true
        let scored_step_names: std::collections::HashSet<&str> = input
            .steps
            .iter()
            .filter(|s| s.evaluate_score)
            .map(|s| s.name.as_str())
            .collect();
        tracing::debug!(
            scored_count = scored_step_names.len(),
            names = ?scored_step_names,
            "Scored evaluation candidates"
        );
        tracing::debug!(
            total_nodes = task_results.len(),
            "Checking scored evaluation targets"
        );
        if let Some(ref se_svc) = self.scored_evaluation_service {
            for tr in &task_results {
                if tr.status != TaskStatus::Success {
                    continue;
                }
                if !scored_step_names.contains(tr.node_name.as_str()) {
                    tracing::debug!(node = %tr.node_name, "Skipping — not in scored steps");
                    continue;
                }
                tracing::debug!(node = %tr.node_name, "Running scored evaluation");
                let node_id = match uuid::Uuid::parse_str(&tr.node_id) {
                    Ok(id) => id,
                    Err(_) => continue,
                };
                let artifact = tr.output.clone().unwrap_or_default();
                let rubric = Rubric::inline(serde_json::json!({
                    "scoring_key": "default"
                }));
                let input = ScoredEvalInput::new(
                    serde_json::Value::String(artifact),
                    rubric,
                    execution_id,
                    node_id,
                    &tr.node_name,
                );
                match se_svc.evaluate(input).await {
                    Ok(output) => {
                        tracing::debug!(
                            node = %tr.node_name,
                            passed = output.result.passed,
                            "Scored evaluation completed"
                        );
                    }
                    Err(e) => {
                        tracing::warn!(
                            node = %tr.node_name,
                            error = %e,
                            "Scored evaluation skipped"
                        );
                    }
                }
            }
        }

        // 4. Determine final status — approval-paused executions are NOT
        // terminal: they are resumable via `approve_execution`.
        let final_status = if approval_pending {
            ExecutionStatus::PendingApproval
        } else if task_results.is_empty() {
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

        // 5. Save final state — for an approval pause, persist the graph +
        // node states + approved set so a DIFFERENT process can resume the
        // run (GAP-3 cross-process resume).
        let save_out = if approval_pending {
            let mut state =
                crate::state_persistence::domain::ExecutionState::new(execution_id, String::new());
            state.status = crate::state_persistence::domain::ExecutionStatus::Pending;
            state.graph = Some(graph_for_resume);
            state.approved = Vec::new();
            state.exec_node_states = Some(exec_output.result.execution_states.clone());
            self.state_manager
                .save_state(state_dto::SaveStateInput { state })
                .await
        } else {
            self.state_manager
                .save_state(Self::make_final_state(
                    execution_id,
                    final_status,
                    Some(&exec_output.result.execution_states),
                ))
                .await
        };
        save_out.map_err(|e| OrchestratorError::StatePersistenceFailed {
            detail: e.to_string(),
            state: format!("{final_status:?}"),
        })?;

        // 6. Quality Gate evaluation
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
                    tracing::info!(%execution_id, quality=%eval_out.summary, "Quality gate evaluated");
                }
            }
        }

        // 7. Policy Engine evaluation
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
                scoring_scores: std::collections::HashMap::new(),
            };
            let eval_policy_input = EvaluatePolicyInput {
                context,
                rule_filter: None,
            };
            if let Ok(eval_policy_out) = policy_svc.evaluate(eval_policy_input).await {
                for action in eval_policy_out.actions {
                    tracing::info!(%execution_id, ?action, "Policy action dispatched");
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
                            crate::event_system::domain::ExecutionEvent::AuditEnvelopeDelivered {
                                timestamp,
                                ..
                            } => *timestamp,
                            crate::event_system::domain::ExecutionEvent::AuditEnvelopeQueued {
                                timestamp,
                                ..
                            } => *timestamp,
                            crate::event_system::domain::ExecutionEvent::AuditEnvelopeDropped {
                                timestamp,
                                ..
                            } => *timestamp,
                            crate::event_system::domain::ExecutionEvent::CircuitBreakerStateChanged {
                                timestamp,
                                ..
                            } => *timestamp,
                            crate::event_system::domain::ExecutionEvent::AuditEnvelopeCreated {
                                timestamp,
                                ..
                            } => *timestamp,
                            | crate::event_system::domain::ExecutionEvent::ApprovalRecorded {
                                timestamp,
                                ..
                            }
                            | crate::event_system::domain::ExecutionEvent::IntentMismatchDetected {
                                timestamp,
                                ..
                            }
                            | crate::event_system::domain::ExecutionEvent::ScopeViolationRecorded {
                                timestamp,
                                ..
                            }
                            | crate::event_system::domain::ExecutionEvent::SequenceRuleMatched {
                                timestamp,
                                ..
                            }
                            | crate::event_system::domain::ExecutionEvent::SequencePolicyDenied {
                                timestamp,
                                ..
                            }
                            | crate::event_system::domain::ExecutionEvent::SequencePolicyConfigError {
                                timestamp,
                                ..
                            } => *timestamp,
                            | crate::event_system::domain::ExecutionEvent::RequirementUnmet {
                                timestamp,
                                ..
                            }
                            | crate::event_system::domain::ExecutionEvent::RequirementPromoted {
                                timestamp,
                                ..
                            } => *timestamp,
                        };
                        ExecutionEventInfo {
                            event_type: pe.event.event_type_name().to_string(),
                            summary: pe.event.event_type_name().to_string(),
                            occurred_at: ts,
                            correlation_id: Some(*pe.event.execution_id()),
                            payload: pe.event.payload_json(),
                            status: pe.event.event_info_status(),
                        }
                    })
                    .collect()
            })
            .unwrap_or_default();

        // 9. Build record
        let (git_commit, git_branch) = Self::detect_git_info(&input.repo_root);
        let record = self.build_record(
            execution_id,
            started_at,
            final_status,
            Some(pmeta),
            task_results,
            ExecutionContext {
                repo_root: input.repo_root,
                symbol_graph_hash: None,
                git_commit,
                git_branch,
                environment: std::env::var("RIGORIX_MCP_SERVER")
                    .map(|_| "rigorix_mcp".to_string())
                    .unwrap_or_else(|_| "rigorix_mcp".to_string()),
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
                .map(|e| {
                    let audit_status = match e.status {
                        EventInfoStatus::Success => crate::audit::domain::EventStatus::Success,
                        EventInfoStatus::Failure => crate::audit::domain::EventStatus::Failure,
                        EventInfoStatus::Info => crate::audit::domain::EventStatus::Success,
                    };
                    crate::audit::domain::ExecutionEventRef {
                        event_type: e.event_type.clone(),
                        summary: e.summary.clone(),
                        occurred_at: e.occurred_at,
                        correlation_id: e.correlation_id,
                        status: audit_status,
                        payload: e.payload.clone(),
                    }
                })
                .collect();
            let _ = audit
                .build_and_send(audit_app::BuildEnvelopeInput {
                    execution_id: record.execution_id,
                    template_id: record.planning.template_id.clone(),
                    planning_prompt: record.planning.prompt_hash.clone(),
                    events: aref,
                    source: Some(record.context.environment.clone()),
                    total_tokens: record.planning.total_tokens,
                    duration_ms: record.duration_ms,
                    git_commit: record.context.git_commit.clone(),
                    git_branch: record.context.git_branch.clone(),
                    model_version: record.planning.model_version.clone(),
                    planning_prompt_content: self.planning_prompt_content(&record.planning),
                    file_paths: Self::extract_file_paths(&record.task_results),
                    metadata: None,
                    scoring_results: self.collect_scoring_results(record.execution_id).await,
                    sign: true,
                    repository: input.repository.clone(),
                    author: input.author.clone(),
                    // L2 (F-20260907-05): stamp the ATTESTED claim on the signed
                    // envelope so cross-run policy history binds by identity
                    // subject (same principal across runs), not caller/git
                    // display strings. Author stays as display-only.
                    identity: input.identity.clone(),
                    effect_key: effect_key::run_effect_key(&record.planning.parameters),
                    // ADR-016: chain link + history-policy opt-in are resolved
                    // by `AuditServiceImpl::build_and_send` before signing.
                    producer_id: None,
                    sequence: None,
                    prev_hash: None,
                    history_policy: None,
                })
                .await;
        }

        // Update state
        if let Some(ref mut s) = *self.current_execution.write().await {
            s.status = final_status;
        }

        // Commit the runbook's budget reservations (1 call per step). The
        // RAII guard (GAP-A-09) reconciles atomically and marks itself
        // committed so its Drop does not roll back a committed reservation.
        for r in &reservations {
            if let Some(ref guard) = r.reservation_guard {
                let _ = guard.commit(1).await;
            } else {
                let _ = self
                    .budget_service
                    .commit(budget_app::CommitReservationInput {
                        execution_id,
                        call_id: r.reservation.call_id,
                        reserved_tokens: r.reservation.reserved_tokens,
                        actual_tokens: 1,
                    })
                    .await;
            }
        }

        tracing::info!(%execution_id, status=?final_status, "run_from_template completed");
        Ok(RunOutput {
            execution_id,
            record,
        })
    }

    async fn plan_from_template(
        &self,
        input: PlanFromTemplateInput,
    ) -> Result<PlanOnlyOutput, OrchestratorError> {
        // L1 identity gate — a preview must show the same refusal the run it
        // precedes would hit (require_identity without attested caller).
        self.enforce_identity_requirement(&input.steps, input.identity.as_ref())?;
        // Same R2 gate as `run_from_template` — a preview must show the same
        // promotion/denial decisions as the run it precedes. L2: principal
        // prefers the attested claim's subject.
        let principal = input
            .identity
            .as_ref()
            .map(|i| i.subject.as_str())
            .or(input.author.as_deref());
        let (gated_steps, findings, requirement_findings) =
            policy_pipeline::PolicyPipeline::new(self)
                .apply(&input.steps, None, principal, input.identity.as_ref())
                .await?;
        let steps: &[super::dto::TemplateStepDef] = &gated_steps;
        let graph = self.build_graph_from_steps(steps)?;
        Ok(PlanOnlyOutput {
            plan: serde_json::json!({
                "template_name": input.template_name,
                "step_count": input.steps.len(),
                "mode": "from_template",
            }),
            graph: serde_json::to_value(&graph).unwrap_or_default(),
            sequence_findings: findings,
            requirement_findings,
        })
    }

    fn event_bus(&self) -> &dyn event_app::EventBusService {
        &*self.event_bus
    }

    async fn approve_execution(
        &self,
        input: ApproveExecutionInput,
    ) -> Result<ApproveExecutionOutput, OrchestratorError> {
        self.approve_execution_flow(input).await
    }

    async fn execution_state(
        &self,
        execution_id: Uuid,
    ) -> Result<exec_dto::GetExecutionStateOutput, OrchestratorError> {
        self.execution_service
            .get_execution_state(exec_dto::GetExecutionStateInput {
                dag_id: execution_id,
            })
            .await
            .map_err(|e| OrchestratorError::Internal {
                detail: format!("Failed to query execution state: {e}"),
                source_module: "orchestrator".into(),
            })
    }
}

// Mocks moved to orchestrator_mocks.rs
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests;

/// L1 identity gate core (F-20260907-05): refuse at plan time when ANY
/// step declares `require_identity = true` and the caller has no attested
/// identity (no claim, or source = `Unverified`). Fail closed — the step
/// never dispatches. Free function so the gate is unit-testable without a
/// full orchestrator instance.
fn check_identity_gate(
    steps: &[super::dto::TemplateStepDef],
    identity: Option<&crate::identity::domain::IdentityRef>,
) -> Result<(), OrchestratorError> {
    let attested = identity
        .filter(|i| {
            !matches!(
                i.source,
                crate::identity::domain::IdentitySource::Unverified
            )
        })
        .is_some();
    for step in steps {
        if step.require_identity && !attested {
            return Err(OrchestratorError::IdentityRequired {
                step: step.name.clone(),
                status: if identity.is_some() {
                    "unverified".to_string()
                } else {
                    "unauthenticated".to_string()
                },
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod identity_gate_tests;
