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
    CheckEnforcementHandler, ExecuteHandler, ValidatePlanHandler,
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
        // Execute the plan through EngineFacade
        let result = self
            .engine
            .execute(input.plan)
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

        let content = serde_json::json!({
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

// ---------------------------------------------------------------------------
// Factory function
// ---------------------------------------------------------------------------

/// Create all three handler instances from shared dependencies.
pub fn create_handler_instances(
    engine: SharedEngineFacade,
    execute_timeout: Duration,
) -> HandlerInstanceSet {
    HandlerInstanceSet {
        execute: Arc::new(ExecuteHandlerImpl::new(engine.clone(), execute_timeout)),
        validate: Arc::new(ValidatePlanHandlerImpl::new(engine.clone())),
        check_enforcement: Arc::new(CheckEnforcementHandlerImpl::new(engine)),
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
}
