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
    ApproveExecutionInput, ApproveExecutionOutput, CancelInput, CancelOutput,
    PlanFromTemplateInput, PlanOnlyInput, PlanOnlyOutput, RunFromTemplateInput, RunInput,
    RunOutput, StatusOutput,
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

    /// Full lifecycle from a pre-resolved template (no intent→plan pipeline).
    ///
    /// Builds a TaskGraph directly from concrete steps, then executes with
    /// the same state persistence, quality gates, policy engine, and audit
    /// dispatch as `run()`. Used when the caller has already resolved the
    /// template (e.g. MCP server loads a .toml file from disk).
    async fn run_from_template(
        &self,
        input: RunFromTemplateInput,
    ) -> Result<RunOutput, OrchestratorError>;

    /// Plan from a pre-resolved template (no execution).
    ///
    /// Builds a TaskGraph from concrete steps and returns it for preview.
    /// Unlike `plan_only()`, this skips intent classification and template
    /// matching — the steps are already concrete.
    async fn plan_from_template(
        &self,
        input: PlanFromTemplateInput,
    ) -> Result<PlanOnlyOutput, OrchestratorError>;

    /// Access the EventBus for subscriber registration (TUI, logs).
    ///
    /// Allows external consumers to subscribe to lifecycle events before a
    /// run starts. The returned reference must be valid for the lifetime of
    /// the service.
    fn event_bus(&self) -> &dyn crate::event_system::application::EventBusService;

    /// Approve steps of an execution paused for human sign-off and resume it.
    ///
    /// Steps that declared `requires_approval: true` are only dispatched
    /// after being approved here. Grants approval by step name, then resumes
    /// the paused execution so the remaining DAG nodes continue. If any steps
    /// remain pending after this call, the execution stays paused.
    async fn approve_execution(
        &self,
        input: ApproveExecutionInput,
    ) -> Result<ApproveExecutionOutput, OrchestratorError>;

    /// Get the current per-node state of an execution (completed/failed/
    /// awaiting-approval). Used after a resumed approval to surface the
    /// FINAL run state (all steps, statuses) for evidence — the paused-run
    /// snapshot alone would show a stale PendingApproval.
    async fn execution_state(
        &self,
        execution_id: uuid::Uuid,
    ) -> Result<crate::execution_engine::application::dto::GetExecutionStateOutput, OrchestratorError>;
}
