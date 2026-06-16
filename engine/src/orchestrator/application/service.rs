//! Service interfaces (use cases) for the Orchestrator bounded context.
//!
//! @canonical .pi/architecture/modules/orchestrator.md#orchestrator-service
//! Implements: Contract Freeze — OrchestratorService trait
//! Issue: #338
//!
//! These traits define the top-level operations for running a full Rigorix
//! execution lifecycle. All methods are async and return domain error types.
//!
//! # Contract (Frozen)
//! - Every use case has a corresponding trait method
//! - Input/output types are DTOs defined in `dto/`
//! - All methods are async (use `async-trait` for trait object safety)
//! - No implementation — only contract signatures

use async_trait::async_trait;

use crate::orchestrator::domain::OrchestratorError;

use super::dto::{
    CancelInput, CancelOutput, PlanOnlyInput, PlanOnlyOutput, RunInput, RunOutput, StatusOutput,
};

/// Single entry point for executing a Rigorix run from intent to result.
///
/// Orchestrates the full lifecycle: planning → DAG execution → state persistence
/// → event emission → audit envelope building.
///
/// Any consumer (CLI, CI/CD, IDE plugin) can run a complete execution with one
/// call by using this trait. Implementations wire together the PlanningPipeline,
/// ParallelExecutionService, StateManagerService, CancellationService, EventBus,
/// and AuditService internally.
#[async_trait]
pub trait OrchestratorService: Send + Sync {
    /// Full lifecycle: plan → execute → persist → emit → return record.
    ///
    /// # Lifecycle
    /// 1. Generate execution_id (UUIDv7)
    /// 2. Publish `PlanningStarted` event
    /// 3. Run `PlanningPipeline::plan_with_graph(intent, budget)`
    /// 4. Publish `PlanningCompleted` event
    /// 5. Save initial `ExecutionState` (Pending)
    /// 6. Execute DAG via `ParallelExecutionService` (cooperative cancellation)
    /// 7. Save final `ExecutionState` (Completed/Failed)
    /// 8. Drain `EventBus` → build `ExecutionRecord`
    /// 9. Send audit envelope (best-effort)
    /// 10. Return `ExecutionRecord`
    ///
    /// # Errors
    /// Returns `OrchestratorError` for any phase failure. The record may be
    /// partially complete depending on when the failure occurred.
    async fn run(&self, input: RunInput) -> Result<RunOutput, OrchestratorError>;

    /// Plan only (no execution). Returns the plan for preview.
    ///
    /// Useful for CLI `--plan` mode where the user wants to review the
    /// generated plan before committing to execution.
    async fn plan_only(&self, input: PlanOnlyInput) -> Result<PlanOnlyOutput, OrchestratorError>;

    /// Cancel a running execution.
    ///
    /// Propagates the cancellation signal to all sub-services via the
    /// `CancellationService`. Once cancelled, the execution enters the
    /// `Cancelled` state and cannot be resumed.
    async fn cancel(&self, input: CancelInput) -> Result<CancelOutput, OrchestratorError>;

    /// Get current execution status.
    ///
    /// Returns the status of the current or most recent execution, including
    /// which DAG nodes have been completed, are running, or are pending.
    async fn status(&self) -> Result<StatusOutput, OrchestratorError>;

    /// Access the EventBus for subscriber registration (TUI, logs).
    ///
    /// Allows external consumers to subscribe to lifecycle events before a
    /// run starts. The returned reference must be valid for the lifetime of
    /// the service.
    fn event_bus(&self) -> &dyn crate::event_system::domain::EventBus;
}
