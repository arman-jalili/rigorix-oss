//! Service interfaces (use cases) for the Execution Tools bounded context.
//!
//! @canonical .pi/architecture/modules/execution-tools.md#services
//! Implements: Contract Freeze — ExecuteHandler, ValidatePlanHandler,
//! CheckEnforcementHandler service traits
//!
//! These traits define the application-level operations for execution tools.
//! All methods are async and return domain error types. Input/output types
//! are DTOs defined in `dto/`.
//!
//! # Contract (Frozen)
//!
//! - Every use case has a corresponding trait method
//! - Input/output types are DTOs
//! - All methods are async (use `async-trait` for trait object safety)
//! - No implementation — only contract signatures
//! - Services are thread-safe (Send + Sync)

use async_trait::async_trait;
use std::time::Duration;

use crate::execution_tools::domain::error::{HandlerError, ToolCallResult};

use super::dto::{ExecuteInput, ValidateInput};

// ---------------------------------------------------------------------------
// ExecuteHandler — Domain Service
// ---------------------------------------------------------------------------

/// Domain service that handles `rigorix_execute` tool calls.
///
/// Validates input, delegates to EngineFacade for execution,
/// and formats the engine result as MCP tool call content.
///
/// # Contract (Frozen)
///
/// - Input validation happens before engine delegate
/// - Timeout enforcement wraps the engine call
/// - Results are formatted as ToolCallResult for MCP response
/// - Never panics — all errors go through HandlerError
#[async_trait]
pub trait ExecuteHandler: Send + Sync {
    /// Handle a `rigorix_execute` tool call.
    ///
    /// 1. Validates the input schema
    /// 2. Executes the plan through EngineFacade with timeout
    /// 3. Formats the execution result as MCP tool call content
    ///
    /// # Errors
    /// - `HandlerError::InvalidArguments` if input fails validation
    /// - `HandlerError::EngineError` if EngineFacade returns an error
    /// - `HandlerError::Timeout` if execution exceeds the configured duration
    async fn handle(&self, input: ExecuteInput) -> Result<ToolCallResult, HandlerError>;

    /// Get the configured execution timeout duration.
    fn timeout_duration(&self) -> Duration;
}

// ---------------------------------------------------------------------------
// ValidatePlanHandler — Domain Service
// ---------------------------------------------------------------------------

/// Domain service that handles `rigorix_validate_plan` tool calls.
///
/// Validates the input, delegates to EngineFacade for plan validation,
/// and formats the validation result as MCP tool call content.
///
/// # Contract (Frozen)
///
/// - Input validation happens before engine delegate
/// - Validation errors are wrapped, not propagated raw
/// - Results are formatted as ToolCallResult for MCP response
#[async_trait]
pub trait ValidatePlanHandler: Send + Sync {
    /// Handle a `rigorix_validate_plan` tool call.
    ///
    /// 1. Validates the input schema
    /// 2. Delegates to EngineFacade for policy validation
    /// 3. Formats the validation result as MCP tool call content
    ///
    /// # Errors
    /// - `HandlerError::InvalidArguments` if input fails validation
    /// - `HandlerError::EngineError` if EngineFacade returns an error
    async fn handle(&self, input: ValidateInput) -> Result<ToolCallResult, HandlerError>;
}

// ---------------------------------------------------------------------------
// CheckEnforcementHandler — Domain Service
// ---------------------------------------------------------------------------

/// Domain service that handles `rigorix_check_enforcement` tool calls.
///
/// Queries the EngineFacade for current enforcement status (budget, limits,
/// circuit breakers) and formats the result as MCP tool call content.
///
/// # Contract (Frozen)
///
/// - Always queries fresh data (never cached)
/// - Results are formatted as ToolCallResult for MCP response
/// - No input parameters needed — operates on current session state
#[async_trait]
pub trait CheckEnforcementHandler: Send + Sync {
    /// Handle a `rigorix_check_enforcement` tool call.
    ///
    /// 1. Queries EngineFacade for fresh enforcement status
    /// 2. Formats the enforcement status as MCP tool call content
    ///
    /// # Errors
    /// - `HandlerError::EngineError` if EngineFacade returns an error
    async fn handle(&self) -> Result<ToolCallResult, HandlerError>;
}

// ---------------------------------------------------------------------------
// PlanHandler — Domain Service
// ---------------------------------------------------------------------------

/// Domain service that handles `rigorix_plan` tool calls.
///
/// Loads a template from the repository, converts it to an execution plan,
/// validates against enforcement policies, and returns a structured DAG
/// plan without executing.
///
/// # Contract (Frozen)
///
/// - Takes the rich template type (with version, tags, timestamps)
/// - Validates enforcement pre-flight via EngineFacade
/// - Returns structured PlanOutput with graph nodes and enforcement status
/// - Never executes — read-only preview
#[async_trait]
pub trait PlanHandler: Send + Sync {
    /// Handle a `rigorix_plan` tool call.
    ///
    /// 1. Converts template_tools::PlanTemplate to execution_tools::PlanTemplate
    /// 2. Validates the plan against enforcement policies via EngineFacade
    /// 3. Builds structured plan output with DAG nodes, constraints, enforcement info
    ///
    /// # Errors
    /// - `HandlerError::InvalidArguments` if template conversion fails
    /// - `HandlerError::EngineError` if EngineFacade validation errors
    async fn handle(
        &self,
        template: &crate::template_tools::domain::value::PlanTemplate,
    ) -> Result<ToolCallResult, HandlerError>;
}
