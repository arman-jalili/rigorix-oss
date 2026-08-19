//! EngineFacadeImpl — concrete implementation of the EngineFacade trait.
//!
//! @canonical .pi/architecture/modules/execution-tools.md#enginefacade-impl
//! Implements: EngineFacade trait — wraps rigorix-engine for execution, validation, enforcement
//!
//! The EngineFacadeImpl delegates execution and plan validation to the
//! OrchestratorService (from-template path), which handles state persistence,
//! DAG execution, quality gates, policy engine, and audit dispatch.
//! Enforcement checks remain direct for check_enforcement().

use async_trait::async_trait;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;
use uuid::Uuid;

use rigorix_engine::enforcement::application::dto::GetBudgetStatusInput;
use rigorix_engine::enforcement::domain::EnforcementError;
use rigorix_engine::orchestrator::application::dto::{
    ApproveExecutionInput, PlanFromTemplateInput, RunFromTemplateInput, RunInput, TemplateStepDef,
};
use rigorix_engine::orchestrator::application::service::OrchestratorService;
use rigorix_engine::orchestrator::domain::OrchestratorError;

use crate::execution_tools::domain::entity::EngineFacade;
use crate::execution_tools::domain::error::EngineFacadeError;
use crate::execution_tools::domain::value::{
    ApprovalResult, BudgetStatus, CostBreakdown, EnforcementStatus, ExecutionId, ExecutionResult,
    ExecutionStatus, PlanTemplate, StepResult, ValidationResult,
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

/// Concrete EngineFacade that wraps rigorix-engine's OrchestratorService.
///
/// Execute() and validate_plan() delegate to the orchestrator's from-template
/// path, which handles the full lifecycle: state persistence, DAG execution,
/// quality gates, policy engine, and audit dispatch.
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

// ── Helpers ──────────────────────────────────────────────────────────────

/// Convert an MCP PlanTemplate to engine TemplateStepDefs.
fn plan_to_step_defs(plan: &PlanTemplate) -> Vec<TemplateStepDef> {
    plan.steps()
        .iter()
        .map(|s| TemplateStepDef {
            name: s.name().to_string(),
            tool: s.tool().to_string(),
            description: s.description().to_string(),
            parameters: s.parameters().clone(),
            // Propagate the plan's frozen-contract approval flag so the
            // engine can pause for human sign-off.
            requires_approval: s.requires_approval(),
            timeout_secs: s.timeout_secs(),
            evaluate_score: s.evaluate_score(),
        })
        .collect()
}

/// Convert engine TaskResult list to MCP StepResult list.
///
/// Non-JSON output (e.g. file contents) is wrapped as a JSON string
/// instead of being silently dropped.
fn task_results_to_steps(
    results: &[rigorix_engine::orchestrator::domain::record::TaskResult],
) -> Vec<StepResult> {
    results
        .iter()
        .map(|tr| {
            let output = tr.output.as_ref().map(|o| {
                // Try to parse as JSON first; if it fails, wrap as string.
                serde_json::from_str(o)
                    .ok()
                    .unwrap_or_else(|| serde_json::Value::String(o.clone()))
            });
            StepResult::new(
                tr.node_name.clone(),
                tr.status == rigorix_engine::orchestrator::domain::record::TaskStatus::Success,
                tr.error.clone(),
                output.unwrap_or(serde_json::Value::Null),
                tr.duration_ms,
            )
        })
        .collect()
}

/// Derive "owner/repo" from `git remote get-url origin` in the given repo root.
fn derive_repository(repo_root: &str) -> Option<String> {
    let output = std::process::Command::new("git")
        .args(["-C", repo_root, "remote", "get-url", "origin"])
        .output()
        .ok()?;
    if output.status.success() {
        let url = String::from_utf8_lossy(&output.stdout).trim().to_string();
        parse_repo_from_url(&url)
    } else {
        None
    }
}

/// Derive author email from `git config user.email` in the given repo root.
fn derive_author(repo_root: &str) -> Option<String> {
    let output = std::process::Command::new("git")
        .args(["-C", repo_root, "config", "user.email"])
        .output()
        .ok()?;
    if output.status.success() {
        let email = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if email.is_empty() { None } else { Some(email) }
    } else {
        None
    }
}

/// Normalize common git remote URL formats to "owner/repo".
fn parse_repo_from_url(url: &str) -> Option<String> {
    let stripped = url
        .trim()
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .or_else(|| url.strip_prefix("ssh://"))
        .unwrap_or(url);

    let path = if let Some(at_pos) = stripped.find('@') {
        if let Some(colon_pos) = stripped[at_pos..].find(':') {
            &stripped[at_pos + colon_pos + 1..]
        } else {
            stripped
        }
    } else if let Some(slash_pos) = stripped.find('/') {
        &stripped[slash_pos + 1..]
    } else {
        stripped
    };

    let path = path.strip_suffix(".git").unwrap_or(path);

    if path.split('/').count() == 2 && !path.is_empty() && !path.starts_with('/') {
        Some(path.to_string())
    } else {
        None
    }
}

fn map_orchestrator_error(err: OrchestratorError) -> EngineFacadeError {
    match &err {
        OrchestratorError::ExecutionFailed { detail, .. } => {
            EngineFacadeError::EngineError(detail.clone())
        }
        OrchestratorError::PlanningFailed { detail, .. } => {
            EngineFacadeError::Internal(format!("Planning failed: {detail}"))
        }
        _ => EngineFacadeError::Internal(err.to_string()),
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

// ── EngineFacade impl ────────────────────────────────────────────────────

#[async_trait]
impl EngineFacade for EngineFacadeImpl {
    async fn execute(
        &self,
        plan: PlanTemplate,
        repository: Option<String>,
        author: Option<String>,
    ) -> Result<ExecutionResult, EngineFacadeError> {
        // Optional enforcement pre-check
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

        // Auto-derive repository and author from git if not provided
        let repository = repository.or_else(|| derive_repository(&self.config.repo_root));
        let author = author.or_else(|| derive_author(&self.config.repo_root));

        let steps_def = plan_to_step_defs(&plan);
        let template_name = plan.name().to_string();

        let input = RunFromTemplateInput {
            steps: steps_def,
            repo_root: self.config.repo_root.clone(),
            execution_id: None, // orchestrator generates UUIDv7
            template_name,
            repository,
            author,
            enforcement_preset: None,
        };

        let run_output = timeout(
            self.config.execute_timeout,
            self.orchestrator.run_from_template(input),
        )
        .await
        .map_err(|_| EngineFacadeError::Timeout {
            operation: "execute".into(),
            duration_secs: self.config.execute_timeout.as_secs(),
        })?
        .map_err(map_orchestrator_error)?;

        let record = &run_output.record;
        let steps = task_results_to_steps(&record.task_results);

        let status = match record.status {
            rigorix_engine::orchestrator::domain::record::ExecutionStatus::Completed => {
                ExecutionStatus::Completed
            }
            rigorix_engine::orchestrator::domain::record::ExecutionStatus::Failed
            | rigorix_engine::orchestrator::domain::record::ExecutionStatus::PartialFailure => {
                ExecutionStatus::Failed
            }
            rigorix_engine::orchestrator::domain::record::ExecutionStatus::Cancelled => {
                ExecutionStatus::Failed
            }
            rigorix_engine::orchestrator::domain::record::ExecutionStatus::PendingApproval => {
                ExecutionStatus::PendingApproval
            }
        };

        let result = ExecutionResult::new(
            run_output.execution_id,
            status,
            steps,
            record.duration_ms,
            None,
            format!("rigorix://audit/{}", run_output.execution_id),
        );

        self.repository.save_execution(&result).await?;
        Ok(result)
    }

    async fn validate_plan(
        &self,
        plan: PlanTemplate,
    ) -> Result<ValidationResult, EngineFacadeError> {
        let steps_def = plan_to_step_defs(&plan);
        let template_name = plan.name().to_string();

        let input = PlanFromTemplateInput {
            steps: steps_def,
            repo_root: self.config.repo_root.clone(),
            template_name,
        };

        let _plan_output = timeout(
            self.config.validate_timeout,
            self.orchestrator.plan_from_template(input),
        )
        .await
        .map_err(|_| EngineFacadeError::Timeout {
            operation: "validate_plan".into(),
            duration_secs: self.config.validate_timeout.as_secs(),
        })?
        .map_err(map_orchestrator_error)?;

        Ok(ValidationResult::new(true, vec![], vec![], None))
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

    async fn run_template(
        &self,
        template_name: &str,
        repository: Option<String>,
        author: Option<String>,
    ) -> Result<ExecutionResult, EngineFacadeError> {
        let repository = repository.or_else(|| derive_repository(&self.config.repo_root));
        let author = author.or_else(|| derive_author(&self.config.repo_root));

        let input = RunInput {
            intent: template_name.to_string(),
            config: serde_json::json!({
                "execution": {
                    "max_llm_calls": 0,
                    "max_llm_tokens": 0,
                }
            }),
            repo_root: self.config.repo_root.clone(),
            repository,
            author,
            enforcement_preset: None,
        };

        let run_output = self
            .orchestrator
            .run(input)
            .await
            .map_err(|e| EngineFacadeError::Internal(e.to_string()))?;

        let record = &run_output.record;
        let steps = task_results_to_steps(&record.task_results);

        let status = match record.status {
            rigorix_engine::orchestrator::domain::record::ExecutionStatus::Completed => {
                ExecutionStatus::Completed
            }
            rigorix_engine::orchestrator::domain::record::ExecutionStatus::Failed
            | rigorix_engine::orchestrator::domain::record::ExecutionStatus::PartialFailure => {
                ExecutionStatus::Failed
            }
            rigorix_engine::orchestrator::domain::record::ExecutionStatus::Cancelled => {
                ExecutionStatus::Failed
            }
            rigorix_engine::orchestrator::domain::record::ExecutionStatus::PendingApproval => {
                ExecutionStatus::PendingApproval
            }
        };

        let result = ExecutionResult::new(
            run_output.execution_id,
            status,
            steps,
            record.duration_ms,
            None,
            format!("rigorix://audit/{}", run_output.execution_id),
        );

        self.repository.save_execution(&result).await?;
        Ok(result)
    }

    async fn approve_execution(
        &self,
        execution_id: &ExecutionId,
        step_names: Vec<String>,
    ) -> Result<ApprovalResult, EngineFacadeError> {
        let output = self
            .orchestrator
            .approve_execution(ApproveExecutionInput {
                execution_id: *execution_id.as_uuid(),
                step_names,
            })
            .await
            .map_err(|e| EngineFacadeError::Internal(e.to_string()))?;

        Ok(ApprovalResult::new(
            output.execution_id,
            output.approved,
            output.not_found,
            output.still_pending,
            output.resumed,
        ))
    }

    async fn execution_state(
        &self,
        execution_id: &ExecutionId,
    ) -> Result<crate::execution_tools::domain::value::ExecutionStateInfo, EngineFacadeError> {
        let state = self
            .orchestrator
            .execution_state(*execution_id.as_uuid())
            .await
            .map_err(|e| EngineFacadeError::Internal(e.to_string()))?;

        use crate::execution_tools::domain::value::{ExecutionStateInfo, NodeExecutionStateInfo};
        let node_states = state
            .node_states
            .into_iter()
            .map(|(id, s)| {
                (
                    id,
                    NodeExecutionStateInfo {
                        node_id: id,
                        node_name: s.node_name.clone(),
                        status: s.status.as_str().to_string(),
                        last_duration_ms: s.last_duration_ms,
                        last_error: s.last_error.clone(),
                    },
                )
            })
            .collect();

        Ok(ExecutionStateInfo {
            execution_id: *execution_id.as_uuid(),
            node_states,
            completed_count: state.completed_count,
            failed_count: state.failed_count,
            skipped_count: state.skipped_count,
            total_nodes: state.total_nodes,
            paused: state.paused,
            is_complete: state.is_complete,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::execution_tools::domain::value::StepDefinition;
    use std::collections::HashMap;

    fn plan_with_approval_flags(approval_flags: &[bool]) -> PlanTemplate {
        let steps: Vec<StepDefinition> = approval_flags
            .iter()
            .enumerate()
            .map(|(i, flag)| {
                StepDefinition::new(
                    format!("step-{}", i),
                    "bash".into(),
                    serde_json::json!({}),
                    *flag,
                    format!("Step {}", i),
                    None,
                )
            })
            .collect();
        PlanTemplate::new(
            "approval-plan".into(),
            "test".into(),
            steps,
            None,
            HashMap::new(),
        )
        .expect("valid plan")
    }

    #[test]
    fn test_plan_to_step_defs_propagates_requires_approval() {
        // Regression test: the facade previously hardcoded
        // requires_approval: false, silently dropping the plan's frozen
        // contract flag so the engine never paused for human sign-off.
        let plan = plan_with_approval_flags(&[false, true, false]);
        let defs = plan_to_step_defs(&plan);

        assert_eq!(defs.len(), 3);
        assert!(
            !defs[0].requires_approval,
            "step-0 must not require approval"
        );
        assert!(
            defs[1].requires_approval,
            "step-1 MUST propagate requires_approval"
        );
        assert!(
            !defs[2].requires_approval,
            "step-2 must not require approval"
        );
    }
}
