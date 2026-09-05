//! ActionRouter implementation — maps GitHub Action events to engine service calls.
//!
//! @canonical actions/.pi/architecture/modules/action-entrypoint.md#actionrouter
//! Implements: ActionRouter trait — dispatches ActionMode variants to engine orchestrator
//!   and validation loop services
//! Issue: issue-actionrouter (#614)
//!
//! The router is stateless — all state lives in the engine. It maps the resolved
//! `ActionMode` and `ActionContext` to the appropriate engine service call and
//! formats the result into `ActionOutput`.

use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use tokio::time::timeout;

use crate::action_entrypoint::domain::{ActionError, ActionMode, ActionOutput};

use super::dto::{DispatchInput, DispatchOutput};
use super::service::ActionRouter;

/// Routes GitHub Action events to engine orchestrator calls.
///
/// Holds references to the engine services it dispatches to.
/// The struct is stateless — all mutable state lives in the engine.
pub struct ActionRouterImpl {
    /// The engine orchestrator service for run, plan_only, and status dispatch.
    orchestrator: Arc<dyn rigorix_engine::orchestrator::application::OrchestratorService>,

    /// Optional validation loop service for Validate mode dispatch.
    /// If `None`, Validate mode falls back to Run mode.
    validation_loop: Option<
        Arc<dyn rigorix_engine::plan_validation::application::service::ValidationLoopService>,
    >,
}

impl ActionRouterImpl {
    /// Create a new ActionRouterImpl with the given engine dependencies.
    ///
    /// # Arguments
    ///
    /// * `orchestrator` — The engine orchestrator service (required)
    /// * `validation_loop` — Optional validation loop service (None = fallback to Run)
    pub fn new(
        orchestrator: Arc<dyn rigorix_engine::orchestrator::application::OrchestratorService>,
        validation_loop: Option<
            Arc<dyn rigorix_engine::plan_validation::application::service::ValidationLoopService>,
        >,
    ) -> Self {
        Self {
            orchestrator,
            validation_loop,
        }
    }

    /// Dispatch to the engine's run mode.
    async fn dispatch_run(
        &self,
        ctx: &crate::action_entrypoint::domain::ActionContext,
    ) -> Result<ActionOutput, ActionError> {
        let intent = ctx
            .mode
            .intent()
            .ok_or_else(|| ActionError::MissingContext {
                detail: "Run mode requires an intent string".to_string(),
                env_var: Some("INPUT_INTENT".to_string()),
            })?;

        let input = rigorix_engine::orchestrator::application::dto::RunInput {
            intent: intent.to_string(),
            repo_root: ctx.workspace_root.clone(),
            config: ctx.to_engine_config(),
            enforcement_preset: None,
            repository: ctx.repository.clone(),
            author: ctx.author.clone(),
            identity: None, // TODO(identity epic): attest from action context
        };

        let output = self
            .orchestrator
            .run(input)
            .await
            .map_err(|e| ActionError::EngineError {
                detail: format!("Orchestrator run failed: {e}"),
                code: None,
            })?;

        Ok(ActionOutput::success(
            format!("Execution completed: {}", output.execution_id),
            Some(output.execution_id.to_string()),
        ))
    }

    /// Dispatch to the engine's plan-only mode.
    async fn dispatch_plan(
        &self,
        ctx: &crate::action_entrypoint::domain::ActionContext,
    ) -> Result<ActionOutput, ActionError> {
        let intent = ctx
            .mode
            .intent()
            .ok_or_else(|| ActionError::MissingContext {
                detail: "Plan mode requires an intent string".to_string(),
                env_var: Some("INPUT_INTENT".to_string()),
            })?;

        let input = rigorix_engine::orchestrator::application::dto::PlanOnlyInput {
            intent: intent.to_string(),
            repo_root: ctx.workspace_root.clone(),
            config: ctx.to_engine_config(),
        };

        let output =
            self.orchestrator
                .plan_only(input)
                .await
                .map_err(|e| ActionError::EngineError {
                    detail: format!("Orchestrator plan_only failed: {e}"),
                    code: None,
                })?;

        // Extract summary from plan output
        let summary = format!(
            "Plan generated with {} nodes",
            output
                .graph
                .as_object()
                .and_then(|m| m.get("nodes"))
                .and_then(|v| v.as_array())
                .map(|a| a.len())
                .unwrap_or(0)
        );

        Ok(ActionOutput::success(summary, None)
            .with_variable("plan", output.plan.to_string())
            .with_variable("graph", output.graph.to_string()))
    }

    /// Dispatch to the engine's validate mode (with optional validation loop).
    async fn dispatch_validate(
        &self,
        ctx: &crate::action_entrypoint::domain::ActionContext,
    ) -> Result<ActionOutput, ActionError> {
        let intent = ctx
            .mode
            .intent()
            .ok_or_else(|| ActionError::MissingContext {
                detail: "Validate mode requires an intent string".to_string(),
                env_var: Some("INPUT_INTENT".to_string()),
            })?;

        // If we have a validation loop service, use it
        if let Some(ref svc) = self.validation_loop {
            let user_intent =
                rigorix_engine::planning::domain::intent::UserIntent::new(intent.to_string(), None);

            let input = rigorix_engine::plan_validation::application::dto::ValidateInput {
                intent: user_intent,
                execution_id: None,
                config:
                    rigorix_engine::plan_validation::domain::loop_config::ValidationLoopConfig {
                        max_iterations: ctx.max_validation_iterations,
                        ..Default::default()
                    },
                existing_template: None,
            };

            let output =
                svc.validate(input)
                    .await
                    .map_err(|e| ActionError::ValidationLoopError {
                        detail: format!("Validation loop failed: {e}"),
                        iterations_completed: None,
                    })?;

            let outcome_str = format!("{:?}", output.outcome);
            Ok(ActionOutput::success(
                format!(
                    "Validation complete: {} ({} iterations, {} failures)",
                    outcome_str, output.iterations, output.total_failures
                ),
                Some(output.execution_id.to_string()),
            ))
        } else {
            // Fallback: run without validation loop
            tracing::warn!("No validation loop service available, falling back to Run mode");
            let run_ctx = ctx.with_mode(ActionMode::Run {
                intent: intent.to_string(),
            });
            self.dispatch_run(&run_ctx).await
        }
    }

    /// Mode A dispatch (GAP-A-08): diff analysis + policy evaluation.
    ///
    /// 1. Requires a `pull_request` event (base branch for the diff)
    /// 2. Computes `git diff {base}...HEAD` in the workspace
    /// 3. Requires the configured policy file to exist (explicit error, not
    ///    a silent fallback)
    /// 4. Runs DiffAnalysisPipeline -> PolicyEvaluationPipeline
    /// 5. `fail_on_violation` gates the action exit code
    async fn dispatch_governance(
        &self,
        ctx: &crate::action_entrypoint::domain::ActionContext,
    ) -> Result<ActionOutput, ActionError> {
        use crate::diff_analyzer::application::service::DiffAnalysisPipelineService;
        use crate::policy_evaluator::application::service::PolicyEvaluationPipelineService;

        // 1. Base branch from the PR event
        let base_branch = match &ctx.event {
            crate::action_entrypoint::domain::GitHubEvent::PullRequest { base_branch, .. } => {
                base_branch.clone()
            }
            _ => return Err(ActionError::GovernanceError {
                detail:
                    "governance mode requires a pull_request event (base branch to diff against)"
                        .to_string(),
            }),
        };

        // 2. Raw diff: git diff {base}...HEAD
        let diff_output = tokio::process::Command::new("git")
            .args(["diff", &format!("{base_branch}...HEAD")])
            .current_dir(&ctx.workspace_root)
            .output()
            .await
            .map_err(|e| ActionError::GovernanceError {
                detail: format!("failed to run git diff: {e}"),
            })?;
        let raw_diff = String::from_utf8_lossy(&diff_output.stdout).to_string();

        // 3. Policy file must exist (AC: explicit error, not silent Validate)
        let policy_path = std::path::Path::new(&ctx.workspace_root).join(&ctx.policy_file);
        if !policy_path.exists() {
            return Err(ActionError::GovernanceError {
                detail: format!(
                    "policy file '{}' not found in workspace (governance mode requires an existing policy file)",
                    ctx.policy_file
                ),
            });
        }

        // 4. Diff analysis -> policy evaluation
        let analyzer = crate::diff_analyzer::application::diff_analysis_pipeline_impl::DiffAnalysisPipelineImpl::default();
        let analyzed =
            analyzer
                .analyze_default(raw_diff)
                .await
                .map_err(|e| ActionError::GovernanceError {
                    detail: format!("diff analysis failed: {e}"),
                })?;

        let evaluator = crate::policy_evaluator::application::policy_evaluation_pipeline_impl::PolicyEvaluationPipelineServiceImpl;
        let (eval_output, report) = evaluator
            .run_with_report(
                crate::policy_evaluator::application::dto::RunPolicyEvaluationInput {
                    diff: analyzed.diff,
                    policy_path: policy_path.to_string_lossy().to_string(),
                    base_ref: base_branch,
                    repo: ctx.repository.clone(),
                    org_policy_config: None,
                    fail_on_violation: ctx.fail_on_violation,
                    include_details: Some(true),
                },
            )
            .await
            .map_err(|e| ActionError::GovernanceError {
                detail: format!("policy evaluation failed: {e}"),
            })?;

        let violations = eval_output.result.violations.len();
        let blocking = eval_output.result.has_blocking_violations;

        // 5. Gate exit code on fail_on_violation
        if ctx.fail_on_violation && blocking {
            return Ok(ActionOutput::failure(format!(
                "Governance: {} blocking policy violations found.\n{}",
                violations, report.markdown_summary
            )));
        }

        Ok(ActionOutput::success(
            format!(
                "Governance: {} violations ({} blocking) — {}",
                violations, blocking, report.markdown_summary
            ),
            None,
        ))
    }

    /// Dispatch to the engine's status mode.
    async fn dispatch_status(
        &self,
        _ctx: &crate::action_entrypoint::domain::ActionContext,
    ) -> Result<ActionOutput, ActionError> {
        let output = self
            .orchestrator
            .status()
            .await
            .map_err(|e| ActionError::EngineError {
                detail: format!("Orchestrator status failed: {e}"),
                code: None,
            })?;

        let summary = format!(
            "Status: {:?} (execution: {})",
            output.status, output.execution_id
        );
        Ok(
            ActionOutput::success(summary, Some(output.execution_id.to_string()))
                .with_variable("execution_id", output.execution_id.to_string())
                .with_variable("status", format!("{:?}", output.status)),
        )
    }
}

#[async_trait]
impl ActionRouter for ActionRouterImpl {
    async fn dispatch(&self, input: DispatchInput) -> Result<DispatchOutput, ActionError> {
        let start = Instant::now();
        let ctx = &input.context;

        // Check if the event is routable unless force is set
        if !input.force && !ctx.event.is_routable() {
            return Ok(DispatchOutput {
                output: ActionOutput::skipped(format!(
                    "Event type '{}' is not routable",
                    ctx.event.event_type()
                )),
                mode: ctx.mode.clone(),
                duration_ms: 0,
                success: false,
            });
        }

        // Dispatch based on mode
        let output = match &ctx.mode {
            ActionMode::Run { .. } => self.dispatch_run(ctx).await,
            ActionMode::Plan { .. } => self.dispatch_plan(ctx).await,
            ActionMode::Validate { .. } => self.dispatch_validate(ctx).await,
            ActionMode::Governance { .. } => self.dispatch_governance(ctx).await,
            ActionMode::Status => self.dispatch_status(ctx).await,
        };

        let duration_ms = start.elapsed().as_millis() as u64;

        match output {
            Ok(output) => Ok(DispatchOutput {
                output,
                mode: ctx.mode.clone(),
                duration_ms,
                success: true,
            }),
            Err(e) => {
                let _is_retriable = e.is_retriable();
                let annotation_level = e.annotation_level();
                let error_output = ActionOutput::failure(format!("{e}")).with_annotation(
                    crate::action_entrypoint::domain::WorkflowAnnotation {
                        level: if annotation_level == "error" {
                            crate::action_entrypoint::domain::AnnotationLevel::Error
                        } else {
                            crate::action_entrypoint::domain::AnnotationLevel::Warning
                        },
                        message: e.to_string(),
                        file: None,
                        line: None,
                        column: None,
                        title: Some(format!("{} dispatch failed", ctx.mode.as_str())),
                    },
                );

                Ok(DispatchOutput {
                    output: error_output,
                    mode: ctx.mode.clone(),
                    duration_ms,
                    success: false,
                })
            }
        }
    }

    async fn dispatch_with_timeout(
        &self,
        input: DispatchInput,
        timeout_secs: u64,
    ) -> Result<DispatchOutput, ActionError> {
        let duration = std::time::Duration::from_secs(timeout_secs);

        match timeout(duration, self.dispatch(input)).await {
            Ok(result) => result,
            Err(_elapsed) => Err(ActionError::EngineError {
                detail: format!("Dispatch timed out after {} seconds", timeout_secs),
                code: Some("TIMEOUT".to_string()),
            }),
        }
    }

    async fn can_handle(&self, mode: &ActionMode) -> bool {
        matches!(
            mode,
            ActionMode::Run { .. }
                | ActionMode::Plan { .. }
                | ActionMode::Validate { .. }
                | ActionMode::Governance { .. }
                | ActionMode::Status
        )
    }

    async fn supported_modes(&self) -> Vec<ActionMode> {
        vec![
            ActionMode::Run {
                intent: String::new(),
            },
            ActionMode::Plan {
                intent: String::new(),
            },
            ActionMode::Validate {
                intent: String::new(),
            },
            ActionMode::Governance {
                intent: String::new(),
            },
            ActionMode::Status,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action_entrypoint::domain::{
        ActionContext, ActionMode, DispatchStatus, GitHubEvent,
    };

    // ── Mock OrchestratorService ──

    struct MockOrchestrator {
        run_should_fail: bool,
        plan_should_fail: bool,
        status_should_fail: bool,
    }

    impl MockOrchestrator {
        fn new() -> Self {
            Self {
                run_should_fail: false,
                plan_should_fail: false,
                status_should_fail: false,
            }
        }

        fn failing() -> Self {
            Self {
                run_should_fail: true,
                plan_should_fail: true,
                status_should_fail: true,
            }
        }
    }

    #[async_trait]
    impl rigorix_engine::orchestrator::application::OrchestratorService for MockOrchestrator {
        async fn run(
            &self,
            _input: rigorix_engine::orchestrator::application::dto::RunInput,
        ) -> Result<
            rigorix_engine::orchestrator::application::dto::RunOutput,
            rigorix_engine::orchestrator::domain::OrchestratorError,
        > {
            if self.run_should_fail {
                return Err(
                    rigorix_engine::orchestrator::domain::OrchestratorError::Internal {
                        detail: "Mock run failure".to_string(),
                        source_module: "mock_orchestrator".to_string(),
                    },
                );
            }
            let exec_id = uuid::Uuid::new_v4();
            let now = chrono::Utc::now();
            Ok(rigorix_engine::orchestrator::application::dto::RunOutput {
                execution_id: exec_id,
                record: rigorix_engine::orchestrator::domain::record::ExecutionRecord {
                    execution_id: exec_id,
                    planning: Default::default(),
                    task_results: vec![],
                    events: vec![],
                    context: Default::default(),
                    started_at: now,
                    completed_at: Some(now),
                    duration_ms: 100,
                    status:
                        rigorix_engine::orchestrator::domain::record::ExecutionStatus::Completed,
                },
            })
        }

        async fn plan_only(
            &self,
            _input: rigorix_engine::orchestrator::application::dto::PlanOnlyInput,
        ) -> Result<
            rigorix_engine::orchestrator::application::dto::PlanOnlyOutput,
            rigorix_engine::orchestrator::domain::OrchestratorError,
        > {
            if self.plan_should_fail {
                return Err(
                    rigorix_engine::orchestrator::domain::OrchestratorError::Internal {
                        detail: "Mock plan failure".to_string(),
                        source_module: "mock_orchestrator".to_string(),
                    },
                );
            }
            Ok(
                rigorix_engine::orchestrator::application::dto::PlanOnlyOutput {
                    plan: serde_json::json!({"steps": []}),
                    graph: serde_json::json!({"nodes": []}),
                    sequence_findings: Vec::new(),
                },
            )
        }

        async fn cancel(
            &self,
            _input: rigorix_engine::orchestrator::application::dto::CancelInput,
        ) -> Result<
            rigorix_engine::orchestrator::application::dto::CancelOutput,
            rigorix_engine::orchestrator::domain::OrchestratorError,
        > {
            Ok(
                rigorix_engine::orchestrator::application::dto::CancelOutput {
                    execution_id: uuid::Uuid::new_v4(),
                    aborted: true,
                    nodes_cancelled: 0,
                },
            )
        }

        async fn status(
            &self,
        ) -> Result<
            rigorix_engine::orchestrator::application::dto::StatusOutput,
            rigorix_engine::orchestrator::domain::OrchestratorError,
        > {
            if self.status_should_fail {
                return Err(
                    rigorix_engine::orchestrator::domain::OrchestratorError::Internal {
                        detail: "Mock status failure".to_string(),
                        source_module: "mock_orchestrator".to_string(),
                    },
                );
            }
            Ok(
                rigorix_engine::orchestrator::application::dto::StatusOutput {
                    execution_id: uuid::Uuid::new_v4(),
                    status:
                        rigorix_engine::orchestrator::domain::record::ExecutionStatus::Completed,
                    nodes: vec![],
                },
            )
        }

        async fn run_from_template(
            &self,
            _input: rigorix_engine::orchestrator::application::dto::RunFromTemplateInput,
        ) -> Result<
            rigorix_engine::orchestrator::application::dto::RunOutput,
            rigorix_engine::orchestrator::domain::OrchestratorError,
        > {
            let exec_id = uuid::Uuid::new_v4();
            let now = chrono::Utc::now();
            Ok(rigorix_engine::orchestrator::application::dto::RunOutput {
                execution_id: exec_id,
                record: rigorix_engine::orchestrator::domain::record::ExecutionRecord {
                    execution_id: exec_id,
                    planning: Default::default(),
                    task_results: vec![],
                    events: vec![],
                    context: Default::default(),
                    started_at: now,
                    completed_at: Some(now),
                    duration_ms: 100,
                    status:
                        rigorix_engine::orchestrator::domain::record::ExecutionStatus::Completed,
                },
            })
        }

        async fn plan_from_template(
            &self,
            _input: rigorix_engine::orchestrator::application::dto::PlanFromTemplateInput,
        ) -> Result<
            rigorix_engine::orchestrator::application::dto::PlanOnlyOutput,
            rigorix_engine::orchestrator::domain::OrchestratorError,
        > {
            Ok(
                rigorix_engine::orchestrator::application::dto::PlanOnlyOutput {
                    plan: serde_json::json!({"mode": "from_template"}),
                    graph: serde_json::json!({"nodes": []}),
                    sequence_findings: Vec::new(),
                },
            )
        }

        fn event_bus(&self) -> &dyn rigorix_engine::event_system::application::EventBusService {
            unimplemented!("event_bus not needed in tests")
        }

        async fn approve_execution(
            &self,
            input: rigorix_engine::orchestrator::application::dto::ApproveExecutionInput,
        ) -> Result<
            rigorix_engine::orchestrator::application::dto::ApproveExecutionOutput,
            rigorix_engine::orchestrator::domain::OrchestratorError,
        > {
            Ok(
                rigorix_engine::orchestrator::application::dto::ApproveExecutionOutput {
                    execution_id: input.execution_id,
                    approved: input.step_names.clone(),
                    not_found: Vec::new(),
                    still_pending: Vec::new(),
                    resumed: true,
                },
            )
        }

        async fn execution_state(
            &self,
            _: uuid::Uuid,
        ) -> Result<
            rigorix_engine::execution_engine::application::dto::GetExecutionStateOutput,
            rigorix_engine::orchestrator::domain::OrchestratorError,
        > {
            unimplemented!("MockOrchestrator::execution_state not needed by current tests")
        }
    }

    // ── Mock ValidationLoopService ──

    struct MockValidationLoop;

    #[async_trait]
    impl rigorix_engine::plan_validation::application::service::ValidationLoopService
        for MockValidationLoop
    {
        async fn validate(
            &self,
            _input: rigorix_engine::plan_validation::application::dto::ValidateInput,
        ) -> Result<
            rigorix_engine::plan_validation::application::dto::ValidateOutput,
            rigorix_engine::plan_validation::domain::error::ValidationLoopError,
        > {
            Ok(rigorix_engine::plan_validation::application::dto::ValidateOutput {
                execution_id: uuid::Uuid::new_v4(),
                outcome: rigorix_engine::plan_validation::domain::outcome::ValidationOutcome::Validated,
                validated_template: None,
                iterations: 1,
                cumulative_tokens: 100,
                total_duration_ms: 500,
                total_failures: 0,
            })
        }

        async fn classify_nodes(
            &self,
            _input: rigorix_engine::plan_validation::application::dto::ClassifyNodesInput,
        ) -> Result<
            rigorix_engine::plan_validation::application::dto::ClassifyNodesOutput,
            rigorix_engine::plan_validation::domain::error::ValidationLoopError,
        > {
            Ok(
                rigorix_engine::plan_validation::application::dto::ClassifyNodesOutput {
                    generative: vec![],
                    deterministic: vec![],
                    total_nodes: 0,
                },
            )
        }

        async fn retry_generative_nodes(
            &self,
            _input: rigorix_engine::plan_validation::application::dto::RetryGenerativeNodesInput,
        ) -> Result<
            rigorix_engine::plan_validation::application::dto::RetryGenerativeNodesOutput,
            rigorix_engine::plan_validation::domain::error::ValidationLoopError,
        > {
            Ok(
                rigorix_engine::plan_validation::application::dto::RetryGenerativeNodesOutput {
                    template: rigorix_engine::templates::domain::Template::default(),
                    retried_count: 0,
                    skipped_count: 0,
                },
            )
        }
    }

    // ── Helpers ──

    fn test_context(mode: ActionMode) -> ActionContext {
        ActionContext::new(
            "/tmp/test-workspace".to_string(),
            GitHubEvent::WorkflowDispatch {
                ref_name: "main".to_string(),
            },
            mode,
            Some("gh_token_123".to_string()),
        )
    }

    fn create_router() -> ActionRouterImpl {
        let orchestrator = Arc::new(MockOrchestrator::new());
        ActionRouterImpl::new(orchestrator, None)
    }

    fn create_router_with_validation() -> ActionRouterImpl {
        let orchestrator = Arc::new(MockOrchestrator::new());
        let validation = Arc::new(MockValidationLoop);
        ActionRouterImpl::new(orchestrator, Some(validation))
    }

    // ── Tests ──

    #[tokio::test]
    async fn test_dispatch_run_success() {
        let router = create_router();
        let ctx = test_context(ActionMode::Run {
            intent: "implement feature X".to_string(),
        });

        let input = DispatchInput {
            context: ctx,
            timeout_secs: None,
            force: false,
        };

        let result = router.dispatch(input).await.unwrap();
        assert!(result.success);
        assert_eq!(result.output.status, DispatchStatus::Success);
        assert!(result.output.execution_id.is_some());
    }

    #[tokio::test]
    async fn test_dispatch_plan_success() {
        let router = create_router();
        let ctx = test_context(ActionMode::Plan {
            intent: "plan feature Y".to_string(),
        });

        let input = DispatchInput {
            context: ctx,
            timeout_secs: None,
            force: false,
        };

        let result = router.dispatch(input).await.unwrap();
        assert!(result.success);
        assert_eq!(result.output.status, DispatchStatus::Success);
        assert!(result.output.output_variables.contains_key("plan"));
        assert!(result.output.output_variables.contains_key("graph"));
    }

    #[tokio::test]
    async fn test_dispatch_status_success() {
        let router = create_router();
        let ctx = test_context(ActionMode::Status);

        let input = DispatchInput {
            context: ctx,
            timeout_secs: None,
            force: false,
        };

        let result = router.dispatch(input).await.unwrap();
        assert!(result.success);
        assert_eq!(result.output.status, DispatchStatus::Success);
        assert!(result.output.output_variables.contains_key("status"));
    }

    #[tokio::test]
    async fn test_dispatch_validate_with_validation_loop() {
        let router = create_router_with_validation();
        let ctx = test_context(ActionMode::Validate {
            intent: "validate feature Z".to_string(),
        });

        let input = DispatchInput {
            context: ctx,
            timeout_secs: None,
            force: false,
        };

        let result = router.dispatch(input).await.unwrap();
        assert!(result.success);
        assert_eq!(result.output.status, DispatchStatus::Success);
        assert!(result.output.execution_id.is_some());
    }

    #[tokio::test]
    async fn test_dispatch_validate_fallback_to_run() {
        // Without validation loop, Validate should fall back to Run
        let router = create_router();
        let ctx = test_context(ActionMode::Validate {
            intent: "validate feature Z".to_string(),
        });

        let input = DispatchInput {
            context: ctx,
            timeout_secs: None,
            force: false,
        };

        let result = router.dispatch(input).await.unwrap();
        assert!(result.success);
        assert_eq!(result.output.status, DispatchStatus::Success);
        assert!(result.output.execution_id.is_some());
    }

    #[tokio::test]
    async fn test_dispatch_skipped_for_non_routable_event() {
        let router = create_router();
        let ctx = ActionContext::new(
            "/tmp/test-workspace".to_string(),
            GitHubEvent::Push {
                branch: "main".to_string(),
                sha: "abc123".to_string(),
                pusher: "test-user".to_string(),
            },
            ActionMode::Status,
            None,
        );

        let input = DispatchInput {
            context: ctx,
            timeout_secs: None,
            force: false,
        };

        let result = router.dispatch(input).await.unwrap();
        assert_eq!(result.output.status, DispatchStatus::Skipped);
    }

    #[tokio::test]
    async fn test_dispatch_forced_for_non_routable_event() {
        let router = create_router();
        let ctx = ActionContext::new(
            "/tmp/test-workspace".to_string(),
            GitHubEvent::Push {
                branch: "main".to_string(),
                sha: "abc123".to_string(),
                pusher: "test-user".to_string(),
            },
            ActionMode::Status,
            None,
        );

        let input = DispatchInput {
            context: ctx,
            timeout_secs: None,
            force: true,
        };

        let result = router.dispatch(input).await.unwrap();
        assert!(result.success);
    }

    #[tokio::test]
    async fn test_dispatch_engine_error_creates_failure_output() {
        let orchestrator = Arc::new(MockOrchestrator::failing());
        let router = ActionRouterImpl::new(orchestrator, None);

        let ctx = test_context(ActionMode::Run {
            intent: "failing task".to_string(),
        });

        let input = DispatchInput {
            context: ctx,
            timeout_secs: None,
            force: false,
        };

        let result = router.dispatch(input).await.unwrap();
        assert!(!result.success);
        assert_eq!(result.output.status, DispatchStatus::Failure);
    }

    #[tokio::test]
    async fn test_dispatch_with_timeout() {
        let router = create_router();
        let ctx = test_context(ActionMode::Status);

        let input = DispatchInput {
            context: ctx,
            timeout_secs: None,
            force: false,
        };

        let result = router.dispatch_with_timeout(input, 30).await.unwrap();
        assert!(result.success);
    }

    #[tokio::test]
    async fn test_can_handle_all_modes() {
        let router = create_router();

        assert!(
            router
                .can_handle(&ActionMode::Run {
                    intent: "test".to_string()
                })
                .await
        );
        assert!(
            router
                .can_handle(&ActionMode::Plan {
                    intent: "test".to_string()
                })
                .await
        );
        assert!(
            router
                .can_handle(&ActionMode::Validate {
                    intent: "test".to_string()
                })
                .await
        );
        assert!(router.can_handle(&ActionMode::Status).await);
    }

    #[tokio::test]
    async fn test_supported_modes_returns_all() {
        let router = create_router();
        let modes = router.supported_modes().await;
        assert_eq!(modes.len(), 5);
        assert!(modes.contains(&ActionMode::Governance {
            intent: String::new()
        }));
    }

    // ── Mode A governance tests (GAP-A-08) ──

    /// Init a throwaway git repo with an optional policy file.
    /// Returns (TempDir, workspace_root) with the base commit on `main`.
    async fn setup_git_workspace(policy_toml: Option<&str>) -> (tempfile::TempDir, String) {
        let dir = tempfile::TempDir::new().unwrap();
        let root = dir.path().to_path_buf();
        let git = |args: Vec<&'static str>| {
            let root = root.clone();
            async move {
                let out = tokio::process::Command::new("git")
                    .args(&args)
                    .current_dir(&root)
                    .output()
                    .await
                    .expect("git must run");
                String::from_utf8_lossy(if out.status.success() {
                    &out.stdout
                } else {
                    &out.stderr
                })
                .to_string()
            }
        };
        git(vec!["init", "-q"]).await;
        git(vec!["checkout", "-q", "-b", "main"]).await;
        git(vec!["config", "user.email", "test@example.com"]).await;
        git(vec!["config", "user.name", "Test"]).await;
        if let Some(policy) = policy_toml {
            std::fs::create_dir_all(root.join(".rigorix")).unwrap();
            std::fs::write(root.join(".rigorix/policy.toml"), policy).unwrap();
        }
        std::fs::write(root.join("README.md"), "base").unwrap();
        git(vec!["add", "-A"]).await;
        git(vec!["commit", "-qm", "base"]).await;
        (dir, root.to_string_lossy().to_string())
    }

    fn governance_context(
        workspace: &str,
        event: GitHubEvent,
        fail_on_violation: bool,
    ) -> ActionContext {
        let mut ctx = ActionContext::new(
            workspace.to_string(),
            event,
            ActionMode::Governance {
                intent: String::new(),
            },
            Some("gh_token_123".to_string()),
        );
        ctx.policy_file = ".rigorix/policy.toml".to_string();
        ctx.fail_on_violation = fail_on_violation;
        ctx
    }

    fn pr_event() -> GitHubEvent {
        GitHubEvent::PullRequest {
            pr_number: 42,
            action: "opened".to_string(),
            title: "test".to_string(),
            base_branch: "main".to_string(),
            head_branch: "feature".to_string(),
            head_sha: "abc123".to_string(),
        }
    }

    #[tokio::test]
    async fn test_dispatch_governance_with_policy_runs_mode_a() {
        let (_dir, workspace) = setup_git_workspace(Some(
            "version = \"1.0.0\"\n\n[[rules.flag]]\nname = \"markdown\"\ndescription = \"md files\"\npattern = \"*.md\"\n",
        ))
        .await;
        let router = create_router();
        let ctx = governance_context(&workspace, pr_event(), false);

        let result = router
            .dispatch(DispatchInput {
                context: ctx,
                timeout_secs: Some(60),
                force: false,
            })
            .await
            .unwrap();

        // Mode A ran (not Validate): output names governance, status success.
        assert!(result.success, "Mode A dispatch must succeed");
        assert!(
            result.output.summary.contains("Governance"),
            "expected Mode A output, got: {}",
            result.output.summary
        );
    }

    #[tokio::test]
    async fn test_dispatch_governance_missing_policy_is_explicit_error() {
        let (_dir, workspace) = setup_git_workspace(None).await;
        let router = create_router();
        let ctx = governance_context(&workspace, pr_event(), false);

        let result = router
            .dispatch(DispatchInput {
                context: ctx,
                timeout_secs: Some(60),
                force: false,
            })
            .await
            .unwrap();

        // dispatch() converts governance errors into a failed output
        // (never a silent Validate fallback).
        assert!(!result.success, "missing policy file must fail the action");
        assert_eq!(result.output.status, DispatchStatus::Failure);
        assert!(
            result.output.summary.contains("policy file"),
            "expected explicit policy-file error, got: {}",
            result.output.summary
        );
    }

    #[tokio::test]
    async fn test_dispatch_governance_requires_pr_event() {
        let (_dir, workspace) = setup_git_workspace(Some("version = \"1.0.0\"\n")).await;
        let router = create_router();
        let ctx = governance_context(
            &workspace,
            GitHubEvent::WorkflowDispatch {
                ref_name: "main".to_string(),
            },
            false,
        );

        let result = router
            .dispatch(DispatchInput {
                context: ctx,
                timeout_secs: Some(60),
                force: false,
            })
            .await
            .unwrap();

        assert!(!result.success);
        assert_eq!(result.output.status, DispatchStatus::Failure);
        assert!(
            result.output.summary.contains("pull_request"),
            "expected pull_request requirement, got: {}",
            result.output.summary
        );
    }

    #[tokio::test]
    async fn test_dispatch_governance_fail_on_violation_gates_exit_code() {
        let (_dir, workspace) = setup_git_workspace(Some(
            "version = \"1.0.0\"\n\n[[rules.deny]]\nname = \"no-sql\"\ndescription = \"No raw SQL\"\npattern = \"*.sql\"\nseverity = \"critical\"\n",
        ))
        .await;
        // Add a violating file on a HEAD branch so `git diff main...HEAD`
        // sees it (the policy file itself stays unchanged from base).
        let root = std::path::PathBuf::from(&workspace);
        let out = tokio::process::Command::new("git")
            .args(["checkout", "-q", "-b", "feature"])
            .current_dir(&root)
            .output()
            .await
            .unwrap();
        assert!(out.status.success());
        std::fs::write(root.join("secret.sql"), "SELECT * FROM users;").unwrap();
        let out = tokio::process::Command::new("git")
            .args(["add", "secret.sql"])
            .current_dir(&root)
            .output()
            .await
            .unwrap();
        assert!(out.status.success());
        let out = tokio::process::Command::new("git")
            .args(["commit", "-qm", "add sql"])
            .current_dir(&root)
            .output()
            .await
            .unwrap();
        assert!(out.status.success());

        let router = create_router();
        let ctx = governance_context(&workspace, pr_event(), true);

        let result = router
            .dispatch(DispatchInput {
                context: ctx,
                timeout_secs: Some(60),
                force: false,
            })
            .await
            .unwrap();

        // fail_on_violation + blocking violation -> Failure status (exit code
        // gated by main.rs on DispatchStatus::Failure).
        assert_eq!(result.output.status, DispatchStatus::Failure);
        assert!(result.output.summary.contains("Governance"));
    }

    #[tokio::test]
    async fn test_dispatch_tracks_duration() {
        let router = create_router();
        let ctx = test_context(ActionMode::Status);

        let input = DispatchInput {
            context: ctx,
            timeout_secs: None,
            force: false,
        };

        let result = router.dispatch(input).await.unwrap();
        // Duration is at least 0ms (may be 0 in extremely fast mock executions)
        assert!(result.success);
    }
}
