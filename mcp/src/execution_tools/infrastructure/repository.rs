//! Repository interfaces for the Execution Tools bounded context.
//!
//! @canonical .pi/architecture/modules/execution-tools.md#repositories
//! Implements: Contract Freeze — ExecutionRepository trait
//!
//! Repositories abstract execution and enforcement state storage behind
//! interfaces, allowing implementations to use in-memory, filesystem,
//! or database storage without coupling domain logic to infrastructure.
//!
//! # Contract (Frozen)
//!
//! - All repository methods are async
//! - All methods return domain error types
//! - No framework-specific annotations on trait definitions
//! - Implementations are hidden behind these interfaces

use async_trait::async_trait;

use crate::execution_tools::domain::error::EngineFacadeError;
use crate::execution_tools::domain::value::{CostBreakdown, ExecutionId, ExecutionResult};

/// Repository for execution results and cost data.
///
/// Abstracts persistence of completed execution records for
/// audit retrieval and cost breakdown queries.
///
/// # Contract (Frozen)
///
/// - `find_execution` returns `None` if no execution with the given ID exists
/// - `save_execution` persists the full execution result
/// - `find_cost_breakdown` derives cost from persisted execution data
/// - Implementations MUST be thread-safe (Send + Sync)
#[async_trait]
pub trait ExecutionRepository: Send + Sync {
    /// Find an execution result by its execution ID.
    ///
    /// Returns `Ok(None)` if no execution with this ID exists.
    async fn find_execution(
        &self,
        execution_id: &ExecutionId,
    ) -> Result<Option<ExecutionResult>, EngineFacadeError>;

    /// Save an execution result for later retrieval.
    async fn save_execution(&self, execution: &ExecutionResult) -> Result<(), EngineFacadeError>;

    /// Get cost breakdown for a completed execution.
    ///
    /// Returns `Ok(None)` if no cost data exists for the given ID.
    async fn find_cost_breakdown(
        &self,
        execution_id: &ExecutionId,
    ) -> Result<Option<CostBreakdown>, EngineFacadeError>;
}
