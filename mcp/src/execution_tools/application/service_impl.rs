//! Concrete implementations of Execution Tools service traits.
//!
//! @canonical .pi/architecture/modules/execution-tools.md#services
//! Implements: ExecuteHandler, ValidatePlanHandler, CheckEnforcementHandler
//!
//! These are the concrete implementations that wire EngineFacade with
//! application logic for execute, validate, and check enforcement use cases.

use async_trait::async_trait;
use std::sync::Arc;
use std::time::Duration;

use crate::execution_tools::application::dto::{ExecuteInput, ValidateInput};
use crate::execution_tools::application::service::{
    CheckEnforcementHandler, ExecuteHandler, PlanHandler, ValidatePlanHandler,
};
use crate::execution_tools::domain::entity::SharedEngineFacade;
use crate::execution_tools::domain::error::{HandlerError, ToolCallResult};

/// Implementation of ExecuteHandler.
pub struct ExecuteHandlerImpl {
    engine: SharedEngineFacade,
    timeout_duration: Duration,
}

impl ExecuteHandlerImpl {
    /// Create a new ExecuteHandlerImpl.
    pub fn new(engine: SharedEngineFacade, timeout_duration: Duration) -> Self {
        Self {
            engine,
            timeout_duration,
        }
    }
}

#[async_trait]
impl ExecuteHandler for ExecuteHandlerImpl {
    async fn handle(&self, input: ExecuteInput) -> Result<ToolCallResult, HandlerError> {
        let plan = input.plan.ok_or_else(|| {
            HandlerError::InvalidArguments(
                "Either 'plan' or 'template_name' must be provided".into(),
            )
        })?;

        // Execute the plan through EngineFacade
        let result = self
            .engine
            .execute(plan, input.repository.clone(), input.author.clone())
            .await
            .map_err(HandlerError::EngineError)?;

        // Format as MCP tool call content
        let content = serde_json::json!({
            "execution_id": result.execution_id(),
            "status": format!("{:?}", result.status()),
            "duration_ms": result.duration_ms(),
            "tokens_used": result.tokens_used(),
            "audit_uri": result.audit_uri(),
            "steps": result.steps().iter().map(|s| {
                serde_json::json!({
                    "step_name": s.step_name(),
                    "success": s.is_success(),
                    "error": s.error(),
                    "duration_ms": s.duration_ms(),
                })
            }).collect::<Vec<_>>(),
        });

        Ok(ToolCallResult {
            content: vec![crate::execution_tools::domain::error::ToolContentItem {
                r#type: "json".into(),
                text: serde_json::to_string_pretty(&content).unwrap_or_else(|_| "{}".to_string()),
            }],
            is_error: false,
        })
    }

    fn timeout_duration(&self) -> Duration {
        self.timeout_duration
    }
}

/// Implementation of ValidatePlanHandler.
pub struct ValidatePlanHandlerImpl {
    engine: SharedEngineFacade,
}

impl ValidatePlanHandlerImpl {
    /// Create a new ValidatePlanHandlerImpl.
    pub fn new(engine: SharedEngineFacade) -> Self {
        Self { engine }
    }
}

#[async_trait]
impl ValidatePlanHandler for ValidatePlanHandlerImpl {
    async fn handle(&self, input: ValidateInput) -> Result<ToolCallResult, HandlerError> {
        let result = self
            .engine
            .validate_plan(input.plan)
            .await
            .map_err(HandlerError::EngineError)?;

        let mut content = serde_json::json!({
            "valid": result.is_valid(),
            "warnings": result.warnings(),
            "errors": result.errors(),
            "estimated_cost": result.estimated_cost().map(|c| {
                serde_json::json!({
                    "estimated_tokens": c.estimated_tokens,
                    "estimated_tool_calls": c.estimated_tool_calls,
                })
            }),
        });

        // Structured sequence-policy findings (R2 plan-time): machine-readable
        // `{rule_id, later_step, action}` entries so a matched sequence is
        // visible to the agent BEFORE a run (module sequence-policy AC#8).
        let findings = result.findings();
        if !findings.is_empty() {
            content["sequence_findings"] = serde_json::to_value(findings).unwrap_or_default();
        }

        Ok(ToolCallResult {
            content: vec![crate::execution_tools::domain::error::ToolContentItem {
                r#type: "json".into(),
                text: serde_json::to_string_pretty(&content).unwrap_or_else(|_| "{}".to_string()),
            }],
            is_error: !result.is_valid(),
        })
    }
}

/// Implementation of CheckEnforcementHandler.
pub struct CheckEnforcementHandlerImpl {
    engine: SharedEngineFacade,
}

impl CheckEnforcementHandlerImpl {
    /// Create a new CheckEnforcementHandlerImpl.
    pub fn new(engine: SharedEngineFacade) -> Self {
        Self { engine }
    }
}

#[async_trait]
impl CheckEnforcementHandler for CheckEnforcementHandlerImpl {
    async fn handle(&self) -> Result<ToolCallResult, HandlerError> {
        let status = self
            .engine
            .check_enforcement()
            .await
            .map_err(HandlerError::EngineError)?;

        let content = serde_json::json!({
            "active": status.is_active(),
            "preset": status.preset(),
            "budget": {
                "tool_calls_total": status.budget().tool_calls_total,
                "tool_calls_remaining": status.budget().tool_calls_remaining,
                "tokens_total": status.budget().tokens_total,
                "tokens_remaining": status.budget().tokens_remaining,
            },
            "circuit_breakers": status.circuit_breakers().iter().map(|cb| {
                serde_json::json!({
                    "name": cb.name,
                    "is_tripped": cb.is_tripped,
                    "description": cb.description,
                })
            }).collect::<Vec<_>>(),
        });

        Ok(ToolCallResult {
            content: vec![crate::execution_tools::domain::error::ToolContentItem {
                r#type: "json".into(),
                text: serde_json::to_string_pretty(&content).unwrap_or_else(|_| "{}".to_string()),
            }],
            is_error: false,
        })
    }
}

/// Implementation of PlanHandler.
pub struct PlanHandlerImpl {
    engine: SharedEngineFacade,
}

impl PlanHandlerImpl {
    /// Create a new PlanHandlerImpl.
    pub fn new(engine: SharedEngineFacade) -> Self {
        Self { engine }
    }
}

#[async_trait]
impl PlanHandler for PlanHandlerImpl {
    async fn handle(
        &self,
        template: &crate::template_tools::domain::value::PlanTemplate,
    ) -> Result<ToolCallResult, HandlerError> {
        // 1. Convert template_tools::PlanTemplate → execution_tools::PlanTemplate via JSON
        let json = serde_json::to_value(template).unwrap_or_default();
        let exec_plan: crate::execution_tools::domain::value::PlanTemplate =
            serde_json::from_value(json).map_err(|e| {
                HandlerError::InvalidArguments(format!("Failed to convert template: {}", e))
            })?;

        // 2. Validate against enforcement policies
        let validation = self
            .engine
            .validate_plan(exec_plan.clone())
            .await
            .map_err(HandlerError::EngineError)?;

        // 3. Build graph nodes from exec_plan steps
        let mut nodes = Vec::new();
        for step in exec_plan.steps() {
            nodes.push(crate::execution_tools::application::dto::GraphNodeInfo {
                name: step.name().to_string(),
                tool: step.tool().to_string(),
                description: step.description().to_string(),
                dependencies: vec![], // current engine: flat, no cross-node deps
            });
        }

        // 4. Build constraints DTO
        let constraints = exec_plan.constraints().map(|c| {
            crate::execution_tools::application::dto::ConstraintsDto {
                max_tool_calls: c.max_tool_calls,
                max_tokens: c.max_tokens,
                max_duration_secs: c.max_duration_secs,
            }
        });

        // 5. Build PlanOutput
        let output = crate::execution_tools::application::dto::PlanOutput {
            template_name: template.name().to_string(),
            description: template.description().to_string(),
            version: template.version().to_string(),
            tags: template.tags().to_vec(),
            graph: crate::execution_tools::application::dto::GraphInfo {
                sealed: true,
                node_count: nodes.len(),
                nodes,
            },
            constraints,
            enforcement: crate::execution_tools::application::dto::PlanEnforcementInfo {
                valid: validation.is_valid(),
                warnings: validation.warnings().to_vec(),
                errors: validation.errors().to_vec(),
                estimated_cost: validation.estimated_cost().map(|c| {
                    crate::execution_tools::application::dto::CostEstimateDto {
                        estimated_tokens: c.estimated_tokens,
                        estimated_tool_calls: c.estimated_tool_calls,
                        estimated_cost_micro: c.estimated_cost_micro,
                    }
                }),
            },
        };

        let content = serde_json::json!(output);

        Ok(ToolCallResult {
            content: vec![crate::execution_tools::domain::error::ToolContentItem {
                r#type: "json".into(),
                text: serde_json::to_string_pretty(&content).unwrap_or_else(|_| "{}".to_string()),
            }],
            is_error: !output.enforcement.valid,
        })
    }
}

// ---------------------------------------------------------------------------
// Factory function
// ---------------------------------------------------------------------------

/// Create all handler instances from shared dependencies.
pub fn create_handler_instances(
    engine: SharedEngineFacade,
    execute_timeout: Duration,
) -> HandlerInstanceSet {
    HandlerInstanceSet {
        execute: Arc::new(ExecuteHandlerImpl::new(engine.clone(), execute_timeout)),
        validate: Arc::new(ValidatePlanHandlerImpl::new(engine.clone())),
        check_enforcement: Arc::new(CheckEnforcementHandlerImpl::new(engine.clone())),
        plan: Arc::new(PlanHandlerImpl::new(engine)),
    }
}

/// Set of all handler instances.
pub struct HandlerInstanceSet {
    /// Execute handler.
    pub execute: Arc<dyn ExecuteHandler>,
    /// Validate plan handler.
    pub validate: Arc<dyn ValidatePlanHandler>,
    /// Check enforcement handler.
    pub check_enforcement: Arc<dyn CheckEnforcementHandler>,
    /// Plan handler.
    pub plan: Arc<dyn PlanHandler>,
}
