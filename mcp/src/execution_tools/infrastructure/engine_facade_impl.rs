//! EngineFacadeImpl — concrete implementation of the EngineFacade trait.
//!
//! @canonical .pi/architecture/modules/execution-tools.md#enginefacade-impl
//! Implements: EngineFacade trait — wraps rigorix-engine for execution, validation, enforcement
//!
//! The EngineFacadeImpl is a thin async facade over rigorix-engine's ParallelExecutionService
//! (for executing plans directly from steps, bypassing intent classification), OrchestratorService
//! (for validate_plan), and ExecutionEnforcer (for budget checks).

use async_trait::async_trait;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;
use uuid::Uuid;

use rigorix_engine::enforcement::application::dto::GetBudgetStatusInput;
use rigorix_engine::enforcement::domain::EnforcementError;
use rigorix_engine::execution_engine::application::dto::ExecuteGraphInput;
use rigorix_engine::execution_engine::application::service::ParallelExecutionService;
use rigorix_engine::orchestrator::application::OrchestratorService;
use rigorix_engine::orchestrator::application::dto::PlanOnlyInput;
use rigorix_engine::orchestrator::domain::OrchestratorError;

use crate::execution_tools::domain::entity::EngineFacade;
use crate::execution_tools::domain::error::EngineFacadeError;
use crate::execution_tools::domain::value::{
    BudgetStatus, CostBreakdown, EnforcementStatus, ExecutionId, ExecutionResult, ExecutionStatus,
    PlanTemplate, StepResult, ValidationResult,
};

use super::repository::ExecutionRepository;

/// Configuration for the EngineFacade implementation.
#[derive(Debug, Clone)]
pub struct EngineFacadeConfig {
    pub execute_timeout: Duration,
    pub validate_timeout: Duration,
    pub enforcement_enabled: bool,
    pub repo_root: String,
}

impl Default for EngineFacadeConfig {
    fn default() -> Self {
        Self {
            execute_timeout: Duration::from_secs(300),
            validate_timeout: Duration::from_secs(60),
            enforcement_enabled: true,
            repo_root: ".".into(),
        }
    }
}

/// Concrete EngineFacade that wraps rigorix-engine services.
///
/// Execute() bypasses the orchestrator's planning pipeline (intent → classify
/// → match template → generate DAG) because the plan already has concrete steps.
/// Instead it builds TaskGraph nodes directly from PlanTemplate steps.
pub struct EngineFacadeImpl {
    orchestrator: Arc<dyn OrchestratorService>,
    execution_engine: Arc<dyn ParallelExecutionService>,
    enforcer: Arc<dyn rigorix_engine::enforcement::application::ExecutionEnforcer>,
    repository: Arc<dyn ExecutionRepository>,
    config: EngineFacadeConfig,
    instance_id: Uuid,
}

impl EngineFacadeImpl {
    pub fn new(
        orchestrator: Arc<dyn OrchestratorService>,
        execution_engine: Arc<dyn ParallelExecutionService>,
        enforcer: Arc<dyn rigorix_engine::enforcement::application::ExecutionEnforcer>,
        repository: Arc<dyn ExecutionRepository>,
        config: EngineFacadeConfig,
    ) -> Self {
        Self {
            orchestrator,
            execution_engine,
            enforcer,
            repository,
            config,
            instance_id: Uuid::new_v4(),
        }
    }

    /// Create a test instance with default configuration.
    #[allow(dead_code)]
    pub fn test_instance(
        orchestrator: Arc<dyn OrchestratorService>,
        execution_engine: Arc<dyn ParallelExecutionService>,
        enforcer: Arc<dyn rigorix_engine::enforcement::application::ExecutionEnforcer>,
    ) -> Self {
        use super::in_memory_repository::InMemoryExecutionRepository;
        Self::new(
            orchestrator,
            execution_engine,
            enforcer,
            Arc::new(InMemoryExecutionRepository::new()),
            EngineFacadeConfig::default(),
        )
    }
}

#[async_trait]
impl EngineFacade for EngineFacadeImpl {
    async fn execute(&self, plan: PlanTemplate) -> Result<ExecutionResult, EngineFacadeError> {
        // Optional enforcement check
        if self.config.enforcement_enabled {
            let _enforcement = self
                .enforcer
                .get_budget_status(GetBudgetStatusInput {
                    execution_id: self.instance_id.to_string(),
                    resources: None,
                })
                .await
                .map_err(map_enforcement_error)?;
        }

        // Build DAG nodes directly from plan steps (bypass intent classification)
        let dag_id = Uuid::new_v4();
        let mut graph = rigorix_engine::dag_engine::domain::TaskGraph::new();

        for step in plan.steps() {
            let node_id = Uuid::new_v4();
            let node = rigorix_engine::dag_engine::domain::TaskNode::new(
                node_id,
                step.name().to_string(),
                step.tool().to_string(),
                vec![], // no cross-node deps (sequential or single-step)
                step.description().to_string(),
            );
            graph.add_unchecked(node).map_err(|e| {
                EngineFacadeError::Internal(format!("Failed to add graph node: {e}"))
            })?;
        }

        graph.seal().map_err(|e| {
            EngineFacadeError::Internal(format!("Failed to seal execution graph: {e}"))
        })?;

        let exec_output = timeout(
            self.config.execute_timeout,
            self.execution_engine
                .execute_graph(ExecuteGraphInput {
                    dag_id,
                    graph: Some(graph),
                    config_override: None,
                }),
        )
        .await
        .map_err(|_| EngineFacadeError::Timeout {
            operation: "execute".into(),
            duration_secs: self.config.execute_timeout.as_secs(),
        })?
        .map_err(|e| EngineFacadeError::EngineError(e.to_string()))?;

        // Convert execution engine results to MCP StepResult list
        let engine_results = &exec_output.result;
        let steps: Vec<StepResult> = engine_results
            .node_results
            .values()
            .map(|tr| {
                StepResult::new(
                    tr.node_name.clone(),
                    tr.success,
                    tr.error.clone(),
                    tr.output
                        .as_ref()
                        .and_then(|o| serde_json::from_str(o).ok())
                        .unwrap_or(serde_json::Value::Null),
                    tr.duration_ms,
                )
            })
            .collect();

        let status = if engine_results.failed_count == 0 && !engine_results.cancelled {
            ExecutionStatus::Completed
        } else {
            ExecutionStatus::Failed
        };

        let result = ExecutionResult::new(
            dag_id,
            status,
            steps,
            engine_results.total_duration_ms,
            None,
            format!("rigorix://audit/{dag_id}"),
        );

        self.repository.save_execution(&result).await?;
        Ok(result)
    }

    async fn validate_plan(
        &self,
        plan: PlanTemplate,
    ) -> Result<ValidationResult, EngineFacadeError> {
        let input = PlanOnlyInput {
            intent: serde_json::to_string(&serde_json::json!({
                "name": plan.name(),
                "step_count": plan.steps().len(),
            }))
            .unwrap_or_default(),
            config: serde_json::json!({ "dry_run": true }),
            repo_root: self.config.repo_root.clone(),
        };

        let output = timeout(
            self.config.validate_timeout,
            self.orchestrator.plan_only(input),
        )
        .await
        .map_err(|_| EngineFacadeError::Timeout {
            operation: "validate_plan".into(),
            duration_secs: self.config.validate_timeout.as_secs(),
        })?
        .map_err(|e| map_orchestrator_error("validate_plan", &e))?;

        let valid = output
            .plan
            .get("valid")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        Ok(ValidationResult::new(valid, vec![], vec![], None))
    }

    async fn check_enforcement(&self) -> Result<EnforcementStatus, EngineFacadeError> {
        let budget_status = self
            .enforcer
            .get_budget_status(GetBudgetStatusInput {
                execution_id: self.instance_id.to_string(),
                resources: None,
            })
            .await
            .map_err(map_enforcement_error)?;

        // Extract tool_calls and tokens budgets from the budgets vec
        let tool_budget = budget_status
            .budgets
            .iter()
            .find(|b| b.resource == "tool_calls");
        let token_budget = budget_status
            .budgets
            .iter()
            .find(|b| b.resource == "tokens");

        Ok(EnforcementStatus::new(
            budget_status.has_exceeded_limits,
            "default".into(),
            BudgetStatus {
                tool_calls_total: tool_budget.map(|b| b.limit).unwrap_or(1000),
                tool_calls_remaining: tool_budget
                    .map(|b| b.limit.saturating_sub(b.used))
                    .unwrap_or(1000),
                tokens_total: token_budget.map(|b| b.limit).unwrap_or(100000),
                tokens_remaining: token_budget
                    .map(|b| b.limit.saturating_sub(b.used))
                    .unwrap_or(100000),
            },
            vec![],
        ))
    }

    async fn get_execution_cost(
        &self,
        execution_id: &ExecutionId,
    ) -> Result<CostBreakdown, EngineFacadeError> {
        self.repository
            .find_cost_breakdown(execution_id)
            .await?
            .ok_or_else(|| EngineFacadeError::ExecutionNotFound(*execution_id.as_uuid()))
    }
}

fn map_orchestrator_error(_operation: &str, err: &OrchestratorError) -> EngineFacadeError {
    match err {
        OrchestratorError::PlanningFailed { detail, .. } => {
            EngineFacadeError::PlanValidationFailed(detail.clone())
        }
        OrchestratorError::ExecutionFailed { detail, .. } => {
            EngineFacadeError::EngineError(detail.clone())
        }
        OrchestratorError::StatePersistenceFailed { detail, .. } => {
            EngineFacadeError::Internal(detail.clone())
        }
        OrchestratorError::CancellationFailed { detail } => {
            EngineFacadeError::Internal(detail.clone())
        }
        OrchestratorError::AuditFailed { detail, .. } => {
            EngineFacadeError::Internal(format!("Audit: {}", detail))
        }
        OrchestratorError::Internal { detail, .. } => EngineFacadeError::Internal(detail.clone()),
    }
}

fn map_enforcement_error(err: EnforcementError) -> EngineFacadeError {
    match err {
        EnforcementError::BudgetExceeded {
            resource,
            used,
            limit,
        } => {
            let tool_calls_remaining = if resource == "tool_calls" {
                limit.saturating_sub(used)
            } else {
                0
            };
            let tokens_remaining = if resource == "tokens" {
                limit.saturating_sub(used)
            } else {
                0
            };
            EngineFacadeError::BudgetExceeded {
                tool_calls_remaining,
                tokens_remaining,
            }
        }
        EnforcementError::ToolBlocked { tool, .. } => {
            EngineFacadeError::EnforcementBlocked(format!("Tool blocked: {}", tool))
        }
        _ => EngineFacadeError::EnforcementBlocked(err.to_string()),
    }
}

