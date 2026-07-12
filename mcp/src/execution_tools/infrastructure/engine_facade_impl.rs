//! EngineFacadeImpl — concrete implementation of the EngineFacade trait.
//!
//! @canonical .pi/architecture/modules/execution-tools.md#enginefacade-impl
//! Implements: EngineFacade trait — wraps rigorix-engine for execution, validation, enforcement
//!
//! The EngineFacadeImpl is a thin async facade over rigorix-engine's OrchestratorService,
//! ExecutionEnforcer, and audit infrastructure.

use async_trait::async_trait;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;
use uuid::Uuid;

use rigorix_engine::enforcement::application::dto::GetBudgetStatusInput;
use rigorix_engine::enforcement::domain::EnforcementError;
use rigorix_engine::orchestrator::application::OrchestratorService;
use rigorix_engine::orchestrator::application::dto::{PlanOnlyInput, RunInput};
use rigorix_engine::orchestrator::domain::OrchestratorError;
use rigorix_engine::orchestrator::domain::record::ExecutionStatus as EngineStatus;

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
pub struct EngineFacadeImpl {
    orchestrator: Arc<dyn OrchestratorService>,
    enforcer: Arc<dyn rigorix_engine::enforcement::application::ExecutionEnforcer>,
    repository: Arc<dyn ExecutionRepository>,
    config: EngineFacadeConfig,
    instance_id: Uuid,
}

impl EngineFacadeImpl {
    pub fn new(
        orchestrator: Arc<dyn OrchestratorService>,
        enforcer: Arc<dyn rigorix_engine::enforcement::application::ExecutionEnforcer>,
        repository: Arc<dyn ExecutionRepository>,
        config: EngineFacadeConfig,
    ) -> Self {
        Self {
            orchestrator,
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
        enforcer: Arc<dyn rigorix_engine::enforcement::application::ExecutionEnforcer>,
    ) -> Self {
        use super::in_memory_repository::InMemoryExecutionRepository;
        Self::new(
            orchestrator,
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

        let input = RunInput {
            intent: serde_json::to_string(&serde_json::json!({
                "name": plan.name(),
                "description": plan.description(),
                "steps": plan.steps().iter().map(|s| s.name()).collect::<Vec<_>>(),
            }))
            .unwrap_or_default(),
            config: serde_json::json!({
                "plan_name": plan.name(),
                "step_count": plan.steps().len(),
            }),
            repo_root: self.config.repo_root.clone(),
            repository: None,
            author: None,
            enforcement_preset: None,
        };

        let output = timeout(self.config.execute_timeout, self.orchestrator.run(input))
            .await
            .map_err(|_| EngineFacadeError::Timeout {
                operation: "execute".into(),
                duration_secs: self.config.execute_timeout.as_secs(),
            })?
            .map_err(|e| map_orchestrator_error("execute", &e))?;

        let record = &output.record;
        let steps: Vec<StepResult> = record
            .task_results
            .iter()
            .map(|t| {
                StepResult::new(
                    t.node_name.clone(),
                    t.status == rigorix_engine::orchestrator::domain::record::TaskStatus::Success,
                    t.error.clone(),
                    t.output
                        .as_ref()
                        .and_then(|o| serde_json::from_str(o).ok())
                        .unwrap_or(serde_json::Value::Null),
                    t.duration_ms,
                )
            })
            .collect();

        let result = ExecutionResult::new(
            record.execution_id,
            map_execution_status(record.status),
            steps,
            record.duration_ms,
            Some(record.planning.total_tokens as u64),
            format!("rigorix://audit/{}", record.execution_id),
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

fn map_execution_status(status: EngineStatus) -> ExecutionStatus {
    match status {
        EngineStatus::Completed => ExecutionStatus::Completed,
        EngineStatus::Failed => ExecutionStatus::Failed,
        EngineStatus::Cancelled => ExecutionStatus::Cancelled,
        EngineStatus::PartialFailure => ExecutionStatus::PartialFailed,
    }
}
