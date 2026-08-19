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
    scored_evaluation_service: Option<Arc<dyn ScoredEvaluationService>>,
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
            scored_evaluation_service: None,
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

    /// Collect scoring results from the scored evaluation service for an execution.
    /// Returns an empty map if the service is not configured or if no results exist.
    async fn collect_scoring_results(
        &self,
        execution_id: Uuid,
    ) -> std::collections::HashMap<String, crate::audit::domain::ScoringResultRef> {
        let Some(ref se_svc) = self.scored_evaluation_service else {
            return std::collections::HashMap::new();
        };
        let Ok(outputs) = se_svc.list_evaluations(execution_id).await else {
            return std::collections::HashMap::new();
        };
        outputs
            .into_iter()
            .map(|output| {
                let ref_map: std::collections::HashMap<
                    String,
                    crate::audit::domain::ScoreDimensionRef,
                > = output
                    .result
                    .dimensions
                    .into_iter()
                    .map(|(k, d)| {
                        (
                            k,
                            crate::audit::domain::ScoreDimensionRef {
                                score: d.score,
                                max: d.max,
                                label: d.label,
                                passed: d.passed,
                            },
                        )
                    })
                    .collect();
                (
                    output.node_id.to_string(),
                    crate::audit::domain::ScoringResultRef {
                        passed: output.result.passed,
                        backend: output.result.backend,
                        dimensions: ref_map,
                        duration_ms: output.result.duration_ms,
                    },
                )
            })
            .collect()
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
                model_version: None,
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
            // Not terminal — persisted as Pending so approve/resume continues it.
            ExecutionStatus::PendingApproval => SpStatus::Pending,
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

    /// Build a sealed TaskGraph directly from pre-resolved template steps.
    ///
    /// Each step becomes a TaskNode with no cross-node dependencies (sequential
    /// or single-step execution). The step's parameters are serialized as the
    /// node's intent string.
    fn build_graph_from_steps(
        &self,
        steps: &[super::dto::TemplateStepDef],
    ) -> Result<crate::dag_engine::domain::TaskGraph, OrchestratorError> {
        let mut graph = crate::dag_engine::domain::TaskGraph::new();
        // Step order is significant (frozen contract, template-tools value.rs):
        // each step depends on its predecessor, so a template executes as a
        // sequential runbook — validate → backup → migrate → verify — instead
        // of a parallel batch where the migrate step could race ahead of the
        // backup step. Parallel DAGs are expressed via explicit dependencies
        // (engine [[nodes]] format); template steps are ordered by definition.
        let mut prev: Option<Uuid> = None;
        for step in steps {
            let node_id = Uuid::new_v4();
            let intent = serde_json::to_string(&step.parameters).unwrap_or_default();
            let deps = prev.map(|p| vec![p]).unwrap_or_default();
            let node = crate::dag_engine::domain::TaskNode::new(
                node_id,
                step.name.clone(),
                step.tool.clone(),
                deps,
                intent,
            )
            .with_requires_approval(step.requires_approval);
            graph
                .add_unchecked(node)
                .map_err(|e| OrchestratorError::Internal {
                    detail: format!("Failed to add graph node: {e}"),
                    source_module: "orchestrator".into(),
                })?;
            prev = Some(node_id);
        }
        graph.seal().map_err(|e| OrchestratorError::Internal {
            detail: format!("Failed to seal graph: {e}"),
            source_module: "orchestrator".into(),
        })?;
        Ok(graph)
    }

    /// Extract file paths from task result outputs.
    /// Uses simple heuristics to find file path patterns in node output text.
    fn extract_file_paths(task_results: &[TaskResult]) -> Vec<String> {
        let mut paths: Vec<String> = Vec::new();
        let path_re = regex::Regex::new(
            r#"(?:^|\s)((?:\.[/\\])?[a-zA-Z0-9_\-./\\]+\.(?:rs|ts|js|py|go|rb|java|kt|swift|c|cpp|h|hpp|toml|json|yaml|yml|md|css|scss|html|svelte|vue))(?::\d+(?::\d+)?)?"#
        ).ok();
        for task in task_results {
            if let Some(ref output) = task.output
                && let Some(ref re) = path_re
            {
                for cap in re.captures_iter(output) {
                    let path = cap.get(1).map(|m| m.as_str().to_string());
                    if let Some(p) = path {
                        // Remove trailing line/column numbers
                        let clean = p.split(':').next().unwrap_or(&p).to_string();
                        if !paths.contains(&clean) {
                            paths.push(clean);
                        }
                    }
                }
            }
        }
        paths
    }

    /// Detect git commit and branch from the working directory.
    fn detect_git_info(repo_root: &str) -> (Option<String>, Option<String>) {
        let run_git = |args: &[&str]| -> Option<String> {
            std::process::Command::new("git")
                .args(args)
                .current_dir(repo_root)
                .output()
                .ok()
                .and_then(|o| {
                    if o.status.success() {
                        Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
                    } else {
                        None
                    }
                })
        };
        let commit = run_git(&["rev-parse", "HEAD"]);
        let branch = run_git(&["rev-parse", "--abbrev-ref", "HEAD"]);
        (commit, branch)
    }

    fn planning_meta(
        pr: &crate::planning::domain::result::PlanningResult,
        graph: Option<&crate::dag_engine::domain::TaskGraph>,
        model_version: &Option<String>,
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
            model_version: model_version.clone(),
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

        let pmeta = Self::planning_meta(
            &plan_out.planning_result,
            Some(&plan_out.graph),
            &self.config.model_version,
        );

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
                    planning_prompt_content: None, // TODO: populate from config when prompt capture is enabled
                    file_paths: Self::extract_file_paths(&record.task_results),
                    metadata: None,
                    scoring_results: self.collect_scoring_results(record.execution_id).await,
                    sign: false,
                    repository: input.repository.clone(),
                    author: input.author.clone(),
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

    // ── From-template methods (skip intent→plan pipeline) ────────────

    async fn run_from_template(
        &self,
        input: RunFromTemplateInput,
    ) -> Result<RunOutput, OrchestratorError> {
        let execution_id = input.execution_id.unwrap_or_else(|| self.gen_id());
        let started_at = chrono::Utc::now();
        tracing::info!(%execution_id, template=%input.template_name, "run_from_template");

        // Init current execution state
        *self.current_execution.write().await = Some(CurrentExecutionState {
            execution_id,
            status: ExecutionStatus::Failed,
            nodes: vec![],
            started_at,
        });

        // 1. Build DAG directly from pre-resolved steps
        let graph = self.build_graph_from_steps(&input.steps)?;
        let node_order: Vec<String> = graph.nodes().map(|n| n.name.clone()).collect();

        let pmeta = PlanningMetadata {
            template_id: input.template_name.clone(),
            confidence: 1.0,
            llm_calls: 0,
            total_tokens: 0,
            prompt_hash: String::new(),
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
            state.exec_node_states =
                Some(exec_output.result.execution_states.clone());
            self.state_manager
                .save_state(state_dto::SaveStateInput { state })
                .await
        } else {
            self.state_manager
                .save_state(Self::make_final_state(execution_id, final_status))
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
                    planning_prompt_content: None, // TODO: populate from config when prompt capture is enabled
                    file_paths: Self::extract_file_paths(&record.task_results),
                    metadata: None,
                    scoring_results: self.collect_scoring_results(record.execution_id).await,
                    sign: false,
                    repository: input.repository.clone(),
                    author: input.author.clone(),
                })
                .await;
        }

        // Update state
        if let Some(ref mut s) = *self.current_execution.write().await {
            s.status = final_status;
        }

        // Commit the runbook's budget reservations (1 call per step).
        for r in &reservations {
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
        let graph = self.build_graph_from_steps(&input.steps)?;
        Ok(PlanOnlyOutput {
            plan: serde_json::json!({
                "template_name": input.template_name,
                "step_count": input.steps.len(),
                "mode": "from_template",
            }),
            graph: serde_json::to_value(&graph).unwrap_or_default(),
        })
    }

    fn event_bus(&self) -> &dyn event_app::EventBusService {
        &*self.event_bus
    }

    async fn approve_execution(
        &self,
        input: ApproveExecutionInput,
    ) -> Result<ApproveExecutionOutput, OrchestratorError> {
        // 0. Cross-process resume (GAP-3): the run paused in another process
        // (or an earlier session). The session is not in this process's
        // memory, so hydrate it from the persisted ExecutionState before
        // approving — the state carries the sealed graph, live node states,
        // and approved set.
        if let Err(crate::execution_engine::domain::ExecutionError::NodeNotFound { node_id }) = self
            .execution_service
            .approve_node(exec_dto::ApproveNodeInput {
                dag_id: input.execution_id,
                step_names: input.step_names.clone(),
            })
            .await
        {
            tracing::info!(
                %node_id,
                "approve_execution: session not in this process — hydrating from persisted state"
            );
                let loaded = self
                    .state_manager
                    .load_state(state_dto::LoadStateInput {
                        execution_id: input.execution_id,
                    })
                    .await
                    .map_err(|e| OrchestratorError::Internal {
                        detail: format!("Failed to load paused execution state: {e}"),
                        source_module: "orchestrator".into(),
                    })?;
                let Some(graph) = loaded.state.graph.clone() else {
                    return Err(OrchestratorError::Internal {
                        detail: format!(
                            "Execution {} has no resumable graph in state",
                            input.execution_id
                        ),
                        source_module: "orchestrator".into(),
                    });
                };
                let node_states = loaded
                    .state
                    .exec_node_states
                    .clone()
                    .unwrap_or_default();
                let approved: std::collections::HashSet<uuid::Uuid> =
                    loaded.state.approved.iter().copied().collect();
                self.execution_service
                    .hydrate_execution(exec_dto::HydrateExecutionInput {
                        dag_id: input.execution_id,
                        graph,
                        node_states,
                        approved,
                    })
                    .await
                    .map_err(|e| OrchestratorError::Internal {
                        detail: format!("Failed to hydrate execution session: {e}"),
                        source_module: "orchestrator".into(),
                    })?;
        }

        // 1. Record approval in the execution engine session (by step name;
        // the engine resolves names to node IDs from its live node states).
        let approve_out = self
            .execution_service
            .approve_node(exec_dto::ApproveNodeInput {
                dag_id: input.execution_id,
                step_names: input.step_names,
            })
            .await
            .map_err(|e| OrchestratorError::Internal {
                detail: format!("Failed to approve execution steps: {e}"),
                source_module: "orchestrator".into(),
            })?;

        // 2. Resume the paused execution if no steps remain pending.
        let mut resumed = false;
        if approve_out.still_pending.is_empty() {
            resumed = self
                .execution_service
                .resume_execution(exec_dto::ResumeExecutionInput {
                    dag_id: input.execution_id,
                })
                .await
                .is_ok();
        }

        // 3. Sync the orchestrator's current execution status.
        if let Some(s) = self.current_execution.write().await.as_mut() {
            s.status = if resumed {
                match self
                    .execution_service
                    .get_execution_state(exec_dto::GetExecutionStateInput {
                        dag_id: input.execution_id,
                    })
                    .await
                {
                    Ok(state) => {
                        let failed = state
                            .node_states
                            .values()
                            .filter(|st| {
                                st.status == crate::execution_engine::domain::NodeStatus::Failed
                            })
                            .count();
                        if failed > 0 {
                            ExecutionStatus::PartialFailure
                        } else {
                            ExecutionStatus::Completed
                        }
                    }
                    Err(_) => ExecutionStatus::Completed,
                }
            } else {
                ExecutionStatus::PendingApproval
            };
        }

        Ok(ApproveExecutionOutput {
            execution_id: input.execution_id,
            approved: approve_out.approved,
            not_found: approve_out.not_found,
            still_pending: approve_out.still_pending,
            resumed,
        })
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
                author: None,
                repository: None,
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
                author: None,
                repository: None,
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
                author: None,
                repository: None,
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
                author: None,
                repository: None,
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
                author: None,
                repository: None,
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
                author: None,
                repository: None,
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

    #[tokio::test]
    async fn test_run_from_template_pauses_for_approval_and_resumes() {
        // Wire a REAL execution service into the orchestrator so the
        // requires_approval gate is exercised end to end (steps → graph →
        // pause → approve → resume).
        use crate::event_system::application::event_bus_service_impl::EventBusServiceImpl;
        use crate::execution_engine::application::service_impl::{
            ParallelExecutionServiceImpl, RetryEvaluationServiceImpl,
        };
        use crate::execution_engine::domain::ParallelExecutorConfig;

        let executor: Arc<dyn exec_svc::ParallelExecutionService> =
            Arc::new(ParallelExecutionServiceImpl::new(
                ParallelExecutorConfig::default(),
                Box::new(RetryEvaluationServiceImpl::new()),
                Arc::new(EventBusServiceImpl::default()),
            ));
        let orch = OrchestratorServiceImpl::new(
            OrchestratorConfig::default(),
            Arc::new(super::super::orchestrator_mocks::MockPlanningService::new()),
            executor,
            Arc::new(super::super::orchestrator_mocks::MockStateService::new()),
            Arc::new(super::super::orchestrator_mocks::MockCancellationService),
            Arc::new(super::super::orchestrator_mocks::MockEventBusService::new()),
            None,
            Arc::new(super::super::orchestrator_mocks::MockBudgetService),
            None,
        );

        let step_def = |name: &str, requires_approval: bool| {
            crate::orchestrator::application::dto::TemplateStepDef {
                name: name.into(),
                tool: "bash".into(),
                description: name.into(),
                parameters: serde_json::json!({}),
                requires_approval,
                timeout_secs: None,
                evaluate_score: false,
            }
        };

        let input = RunFromTemplateInput {
            steps: vec![step_def("build", false), step_def("deploy", true)],
            repo_root: "/tmp/t".into(),
            execution_id: None,
            template_name: "approval-test".into(),
            repository: None,
            author: None,
            enforcement_preset: None,
        };

        // 1. First run pauses at the approval boundary — NOT terminal.
        let out = orch.run_from_template(input).await.unwrap();
        assert_eq!(
            out.record.status,
            ExecutionStatus::PendingApproval,
            "record must report PendingApproval, got {:?}",
            out.record.status
        );
        assert!(
            !out.record.status.is_terminal(),
            "pending approval is resumable"
        );

        // 2. Approve the deploy step → execution resumes and completes.
        let approve = orch
            .approve_execution(ApproveExecutionInput {
                execution_id: out.execution_id,
                step_names: vec!["deploy".into()],
            })
            .await
            .unwrap();
        assert_eq!(approve.approved, vec!["deploy".to_string()]);
        assert!(approve.still_pending.is_empty());
        assert!(approve.resumed, "execution should resume after approval");
    }

    #[test]
    fn test_build_graph_from_steps_chains_sequentially() {
        // Frozen contract (template-tools value.rs): "Step order is significant".
        // Steps must execute as a sequential runbook, not a parallel batch — a
        // migration template (validate → backup → migrate → verify) would
        // otherwise race the destructive step ahead of the backup.
        let orch = OrchestratorServiceImpl::default_test();
        let steps = [
            crate::orchestrator::application::dto::TemplateStepDef {
                name: "validate".into(),
                tool: "bash".into(),
                description: "validate".into(),
                parameters: serde_json::json!({}),
                requires_approval: false,
                timeout_secs: None,
                evaluate_score: false,
            },
            crate::orchestrator::application::dto::TemplateStepDef {
                name: "backup".into(),
                tool: "bash".into(),
                description: "backup".into(),
                parameters: serde_json::json!({}),
                requires_approval: false,
                timeout_secs: None,
                evaluate_score: false,
            },
            crate::orchestrator::application::dto::TemplateStepDef {
                name: "migrate".into(),
                tool: "bash".into(),
                description: "migrate".into(),
                parameters: serde_json::json!({}),
                requires_approval: true,
                timeout_secs: None,
                evaluate_score: false,
            },
        ];
        let graph = orch.build_graph_from_steps(&steps).unwrap();
        let nodes: Vec<_> = graph.nodes().collect();
        assert_eq!(nodes.len(), 3, "all three steps must be in the graph");

        // First step has no dependency; every later step depends on its
        // immediate predecessor (and transitively on all earlier steps).
        assert!(
            nodes[0].dependencies.is_empty(),
            "first step must have no dependencies"
        );
        assert_eq!(
            nodes[1].dependencies,
            vec![nodes[0].id],
            "step 2 must depend on step 1"
        );
        assert_eq!(
            nodes[2].dependencies,
            vec![nodes[1].id],
            "step 3 must depend on step 2"
        );

        // Approval flag survives the graph construction.
        assert!(!nodes[0].requires_approval());
        assert!(!nodes[1].requires_approval());
        assert!(
            nodes[2].requires_approval(),
            "requires_approval must propagate to the graph node"
        );
    }

    #[tokio::test]
    async fn test_run_from_template_refused_when_budget_exhausted() {
        // Budget-halt (spike's second half): a tight budget must refuse the
        // runbook BEFORE execution — deterministically, not halfway through a
        // consequential operation.
        use crate::budget_tracking::application::llm_budget_impl::LlmBudgetImpl;
        use crate::event_system::application::event_bus_service_impl::EventBusServiceImpl;
        use crate::execution_engine::application::service_impl::{
            ParallelExecutionServiceImpl, RetryEvaluationServiceImpl,
        };
        use crate::execution_engine::domain::ParallelExecutorConfig;

        let executor: Arc<dyn exec_svc::ParallelExecutionService> =
            Arc::new(ParallelExecutionServiceImpl::new(
                ParallelExecutorConfig::default(),
                Box::new(RetryEvaluationServiceImpl::new()),
                Arc::new(EventBusServiceImpl::default()),
            ));
        // Budget allows only 2 calls; the runbook needs 3.
        let budget: Arc<dyn budget_app::LlmBudgetService> =
            Arc::new(LlmBudgetImpl::new(2, 1000, "test".into()));
        let orch = OrchestratorServiceImpl::new(
            OrchestratorConfig::default(),
            Arc::new(super::super::orchestrator_mocks::MockPlanningService::new()),
            executor,
            Arc::new(super::super::orchestrator_mocks::MockStateService::new()),
            Arc::new(super::super::orchestrator_mocks::MockCancellationService),
            Arc::new(super::super::orchestrator_mocks::MockEventBusService::new()),
            None,
            budget,
            None,
        );

        let step_def = |name: &str| crate::orchestrator::application::dto::TemplateStepDef {
            name: name.into(),
            tool: "bash".into(),
            description: name.into(),
            parameters: serde_json::json!({}),
            requires_approval: false,
            timeout_secs: None,
            evaluate_score: false,
        };
        let input = RunFromTemplateInput {
            steps: vec![step_def("a"), step_def("b"), step_def("c")],
            repo_root: "/tmp/t".into(),
            execution_id: None,
            template_name: "budget-test".into(),
            repository: None,
            author: None,
            enforcement_preset: None,
        };

        let err = orch.run_from_template(input).await.unwrap_err();
        match err {
            OrchestratorError::Internal { detail, .. } => {
                assert!(
                    detail.contains("Budget exhausted"),
                    "expected budget-exhausted refusal, got: {detail}"
                );
                assert!(detail.contains("3 steps"), "should name the runbook size");
                assert!(detail.contains("2 of 2 calls"), "should name the budget");
            }
            e => panic!("expected Internal budget error, got {e:?}"),
        }
    }
}
