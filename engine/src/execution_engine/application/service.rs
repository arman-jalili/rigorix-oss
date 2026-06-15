//! Service interfaces (use cases) for the Execution Engine bounded context.
//!
//! @canonical .pi/architecture/modules/execution-engine.md
//! Implements: Contract Freeze — ParallelExecutionService and RetryEvaluationService traits
//! Issue: issue-contract-freeze
//!
//! These traits define the application-level operations for parallel DAG execution
//! and retry logic:
//! - `ParallelExecutionService`: Execute sealed DAGs in parallel with retry support
//! - `RetryEvaluationService`: Evaluate retry policies and make retry decisions
//!
//! # Contract (Frozen)
//! - Every use case has a corresponding trait method
//! - Input/output types are DTOs defined in `dto/`
//! - All methods are async (use `async-trait` for trait object safety)
//! - No implementation — only contract signatures

use async_trait::async_trait;
use uuid::Uuid;

use crate::execution_engine::domain::{
    ExecutionError, FailureContext, NodeExecutionState, RetryDecision,
};

use super::dto::{
    AbortExecutionInput, AbortExecutionOutput, EvaluateRetryInput, EvaluateRetryOutput,
    ExecuteGraphInput, ExecuteGraphOutput, ExecuteNodeInput, ExecuteNodeOutput,
    GetExecutionStateInput, GetExecutionStateOutput, PauseExecutionInput, PauseExecutionOutput,
    ResumeExecutionInput, ResumeExecutionOutput,
};

/// Parallel DAG execution service.
///
/// The ParallelExecutionService orchestrates the concurrent execution of
/// nodes in a sealed TaskGraph. It manages:
///
/// 1. Reading the ready queue and dispatching nodes to concurrent workers
/// 2. Collecting results and updating the graph's execution state
/// 3. Handling retries via the RetryEvaluationService
/// 4. Managing cancellation signals and enforcement limits
/// 5. Emitting execution events to the event bus
///
/// # Execution Lifecycle
///
/// 1. `execute_graph` — Start executing a sealed graph (non-blocking or blocking)
/// 2. `execute_node` — Execute a single node (used by the internal dispatch loop)
/// 3. `get_execution_state` — Poll the current state of an in-flight execution
/// 4. `pause_execution` / `resume_execution` — Pause/resume an in-flight execution
/// 5. `abort_execution` — Abort an in-flight execution
///
/// # Parallelism Model
///
/// The executor uses tokio's `JoinSet` to manage concurrent node execution.
/// The `max_concurrent_executions` config controls how many nodes run at once.
/// Ready nodes are dequeued from the TaskGraph's ready queue and dispatched
/// to the JoinSet up to the concurrency limit.
///
/// # Cancellation Integration
///
/// The executor checks the `CancellationToken` before dispatching each node
/// and between retry attempts. When cancellation is received:
/// - No new nodes are dispatched
/// - In-flight nodes are allowed to complete (graceful shutdown)
/// - The execution result is marked as cancelled
#[async_trait]
pub trait ParallelExecutionService: Send + Sync {
    /// Execute a sealed TaskGraph from end to end.
    ///
    /// Dispatches nodes from the ready queue to the executor's worker pool,
    /// respecting the concurrency limit. Returns when all nodes have reached
    /// a terminal state or the execution is aborted/cancelled.
    ///
    /// # Errors
    /// - `ExecutionError::GraphNotSealed` if the graph has not been sealed
    /// - `ExecutionError::ExecutionCancelled` if cancellation was received
    /// - `ExecutionError::EnforcementRejected` if enforcement limits exceeded
    async fn execute_graph(
        &self,
        input: ExecuteGraphInput,
    ) -> Result<ExecuteGraphOutput, ExecutionError>;

    /// Execute a single node (used internally by the dispatch loop).
    ///
    /// Runs the node's tool binding, applies the retry policy on failure,
    /// and returns the execution result. This is the unit of work dispatched
    /// to the JoinSet.
    async fn execute_node(
        &self,
        input: ExecuteNodeInput,
    ) -> Result<ExecuteNodeOutput, ExecutionError>;

    /// Get the current execution state of a DAG execution.
    ///
    /// Returns the per-node execution states and aggregate summary for
    /// monitoring and progress tracking.
    async fn get_execution_state(
        &self,
        input: GetExecutionStateInput,
    ) -> Result<GetExecutionStateOutput, ExecutionError>;

    /// Pause an in-flight execution.
    ///
    /// In-flight nodes are allowed to complete, but no new nodes are dispatched
    /// from the ready queue until `resume_execution` is called.
    async fn pause_execution(
        &self,
        input: PauseExecutionInput,
    ) -> Result<PauseExecutionOutput, ExecutionError>;

    /// Resume a paused execution.
    async fn resume_execution(
        &self,
        input: ResumeExecutionInput,
    ) -> Result<ResumeExecutionOutput, ExecutionError>;

    /// Abort an in-flight execution.
    ///
    /// Cancels all in-flight nodes via cancellation token and marks remaining
    /// ready/pending nodes as skipped. Produces a terminal ExecutionResult.
    async fn abort_execution(
        &self,
        input: AbortExecutionInput,
    ) -> Result<AbortExecutionOutput, ExecutionError>;

    /// Register a callback for execution progress notifications.
    ///
    /// The callback is invoked each time a node reaches a terminal state
    /// (Completed, Failed, Skipped). Useful for TUI updates and logging.
    fn on_progress(&self, callback: Box<dyn Fn(ExecutionProgress) + Send + Sync>);
}

/// Progress notification for an in-flight execution.
///
/// Emitted each time a node reaches a terminal state.
#[derive(Debug, Clone)]
pub struct ExecutionProgress {
    /// The DAG execution ID.
    pub dag_id: Uuid,
    /// The node that reached a terminal state.
    pub node_id: Uuid,
    /// The current state of the node.
    pub state: NodeExecutionState,
    /// Total number of nodes in the graph.
    pub total_nodes: u32,
    /// Number of completed nodes so far.
    pub completed_count: u32,
    /// Number of failed nodes so far.
    pub failed_count: u32,
    /// Number of skipped nodes so far.
    pub skipped_count: u32,
}

/// Retry evaluation service.
///
/// The RetryEvaluationService evaluates RetryPolicy configurations against
/// FailureContexts to produce RetryDecisions. It answers:
/// - Should this node be retried given the failure type and attempt count?
/// - What strategy should the retry use?
/// - How long should we wait before retrying?
/// - If retries are exhausted, should we use fallback, skip, or abort?
///
/// # Contract (Frozen)
/// - Retry decisions are purely policy-driven
/// - Backoff is computed based on the configured BackoffStrategy
/// - Fallback is only considered after max_attempts is exhausted
/// - The service is stateless (decisions are purely computational)
#[async_trait]
pub trait RetryEvaluationService: Send + Sync {
    /// Evaluate whether a failed node should be retried.
    ///
    /// Given the failure context and retry policy, determine the next
    /// action: retry, fallback, skip, or abort.
    async fn evaluate_retry(
        &self,
        input: EvaluateRetryInput,
    ) -> Result<EvaluateRetryOutput, ExecutionError>;

    /// Compute the backoff delay for a retry attempt.
    ///
    /// Delegates to the policy's BackoffStrategy to compute the delay.
    async fn compute_backoff(
        &self,
        failure_context: &FailureContext,
        policy: &crate::execution_engine::domain::RetryPolicy,
    ) -> u64;

    /// Validate a RetryPolicy configuration.
    ///
    /// Checks:
    /// - max_attempts >= 1 (at least one execution attempt)
    /// - BackoffStrategy parameters are valid (multiplier >= 1.0, etc.)
    /// - retry_strategies is not empty
    async fn validate_policy(
        &self,
        policy: &crate::execution_engine::domain::RetryPolicy,
    ) -> Result<Vec<String>, ExecutionError>;

    /// Determine if a failure type is retriable under the given policy.
    async fn is_failure_retriable(
        &self,
        policy: &crate::execution_engine::domain::RetryPolicy,
        failure_type: &str,
    ) -> bool;

    /// Make a retry decision from a FailureContext (synchronous helper).
    ///
    /// This is the core decision function. It follows this logic:
    /// 1. Check if the failure type is retriable (if policy specifies filters)
    /// 2. Check if max_attempts is exhausted
    /// 3. If retriable and not exhausted → Retry with strategy + backoff
    /// 4. If exhausted with fallback enabled → Fallback
    /// 5. If exhausted and skip_on_exhaustion → Skip
    /// 6. Otherwise → Abort
    async fn decide(
        &self,
        failure_context: &FailureContext,
        policy: &crate::execution_engine::domain::RetryPolicy,
        fallback_node_id: Option<Uuid>,
    ) -> RetryDecision;
}
