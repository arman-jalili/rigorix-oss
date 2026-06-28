//! Core parallel execution data structures: ParallelExecutorConfig, NodeExecutionState,
//! ExecutionResult, TaskResult.
//!
//! @canonical .pi/architecture/modules/execution-engine.md#parallel-executor
//! Implements: Contract Freeze — ParallelExecutorConfig, NodeExecutionState,
//! ExecutionResult, TaskResult
//! Issue: issue-contract-freeze
//!
//! Defines the core parallel execution data structures for the execution engine:
//! - `ParallelExecutorConfig`: Configuration for the parallel executor
//! - `NodeExecutionState`: Runtime state of a single node during execution
//! - `ExecutionResult`: Aggregate result of a full DAG execution
//! - `TaskResult`: Result of a single node execution
//! - `NodeStatus`: Status lifecycle of a node during execution
//!
//! # Contract (Frozen)
//! - ParallelExecutor manages concurrent node execution via tokio JoinSet
//! - NodeExecutionState tracks per-node lifecycle (Pending → Running → Terminal)
//! - ExecutionResult aggregates all node results
//! - TaskResult carries output, duration, and status

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use super::retry::RetryPolicy;

// ---------------------------------------------------------------------------
// ParallelExecutorConfig — Configuration for the Parallel Executor
// ---------------------------------------------------------------------------

/// Configuration for the parallel executor.
///
/// Controls the degree of parallelism, backoff settings, and integration
/// points for cancellation, enforcement, and risk gating.
///
/// # Contract (Frozen)
/// - `max_concurrent_executions`: Maximum number of nodes executing concurrently
/// - `default_retry_policy`: Fallback retry policy when a node has no policy
/// - `enable_cancellation`: Whether cancellation signals are checked
/// - `enable_enforcement`: Whether enforcement limits are applied
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParallelExecutorConfig {
    /// Maximum number of nodes executing concurrently.
    /// Default: 4. A value of 0 means unlimited (all ready nodes execute at once).
    pub max_concurrent_executions: u32,

    /// Fallback retry policy for nodes that don't specify their own.
    pub default_retry_policy: RetryPolicy,

    /// Whether to check cancellation signals during execution.
    pub enable_cancellation: bool,

    /// Whether to enforce execution limits (concurrency, total operations).
    pub enable_enforcement: bool,

    /// Maximum number of node retries per graph execution session.
    /// Prevents infinite retry loops across the entire execution.
    pub max_total_retries_per_session: u32,

    /// Maximum number of node failures allowed before aborting the execution.
    /// 0 means unlimited failures (continue until all nodes are terminal).
    pub max_failures_before_abort: u32,

    /// Whether to execute fallback nodes when retry limits are exhausted.
    pub enable_fallback: bool,

    /// Whether to run post-execution validation rules on nodes.
    pub enable_validation: bool,
}

impl Default for ParallelExecutorConfig {
    fn default() -> Self {
        Self {
            max_concurrent_executions: 4,
            default_retry_policy: RetryPolicy::default(),
            enable_cancellation: true,
            enable_enforcement: true,
            max_total_retries_per_session: 100,
            max_failures_before_abort: 0, // Unlimited by default
            enable_fallback: true,
            enable_validation: true,
        }
    }
}

// ---------------------------------------------------------------------------
// NodeStatus — Lifecycle Status of a Node
// ---------------------------------------------------------------------------

/// Lifecycle status of a node during execution.
///
/// # Contract (Frozen)
/// - `Pending`: Waiting for dependencies to complete
/// - `Ready`: All dependencies satisfied, waiting for executor slot
/// - `Running`: Currently executing
/// - `Completed`: Execution succeeded
/// - `Failed`: Execution failed, retries exhausted
/// - `Skipped`: Skipped via retry strategy or configuration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NodeStatus {
    /// Waiting for dependencies to complete.
    Pending,
    /// All dependencies satisfied, waiting for an executor slot.
    Ready,
    /// Currently executing.
    Running,
    /// Execution succeeded.
    Completed,
    /// Execution failed, all retries exhausted.
    Failed,
    /// Skipped via retry strategy or configuration.
    Skipped,
}

impl NodeStatus {
    /// Returns true if the node is in a terminal state.
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            NodeStatus::Completed | NodeStatus::Failed | NodeStatus::Skipped
        )
    }

    /// Returns true if the node can transition to `Running`.
    pub fn can_execute(&self) -> bool {
        matches!(self, NodeStatus::Ready)
    }

    /// Returns the canonical snake_case name of this status.
    pub fn as_str(&self) -> &'static str {
        match self {
            NodeStatus::Pending => "pending",
            NodeStatus::Ready => "ready",
            NodeStatus::Running => "running",
            NodeStatus::Completed => "completed",
            NodeStatus::Failed => "failed",
            NodeStatus::Skipped => "skipped",
        }
    }
}

// ---------------------------------------------------------------------------
// NodeExecutionState — Runtime State of a Single Node
// ---------------------------------------------------------------------------

/// Runtime execution state of a single node within a parallel execution.
///
/// Tracks the lifecycle, retry attempts, timing, and outcome of a node
/// as it moves through the parallel executor.
///
/// # Contract (Frozen)
/// - State transitions follow: Pending → Ready → Running → Completed|Failed|Skipped
/// - Retry transitions: Failed → Running (if retry policy allows)
/// - All timestamps are UTC
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeExecutionState {
    /// The node's UUID.
    pub node_id: Uuid,

    /// The node's human-readable name.
    pub node_name: String,

    /// Current lifecycle status.
    pub status: NodeStatus,

    /// Number of retry attempts made (0 = first attempt, 1 = first retry, etc.).
    pub retry_attempts: u8,

    /// Duration of the last execution attempt in milliseconds.
    pub last_duration_ms: Option<u64>,

    /// Total accumulated execution time across all attempts in milliseconds.
    pub total_duration_ms: u64,

    /// ISO 8601 timestamp when the node entered Ready state.
    pub ready_at: Option<DateTime<Utc>>,

    /// ISO 8601 timestamp of the most recent execution start.
    pub started_at: Option<DateTime<Utc>>,

    /// ISO 8601 timestamp when the node reached a terminal state.
    pub completed_at: Option<DateTime<Utc>>,

    /// The error message from the last failure (if failed).
    pub last_error: Option<String>,

    /// The failure type from the last failure (if failed).
    pub last_failure_type: Option<String>,
}

impl NodeExecutionState {
    /// Create a new NodeExecutionState in Pending state.
    pub fn new(node_id: Uuid, node_name: impl Into<String>) -> Self {
        Self {
            node_id,
            node_name: node_name.into(),
            status: NodeStatus::Pending,
            retry_attempts: 0,
            last_duration_ms: None,
            total_duration_ms: 0,
            ready_at: None,
            started_at: None,
            completed_at: None,
            last_error: None,
            last_failure_type: None,
        }
    }

    /// Transition the node to Ready state.
    pub fn mark_ready(&mut self) {
        self.status = NodeStatus::Ready;
        self.ready_at = Some(Utc::now());
    }

    /// Transition the node to Running state.
    pub fn mark_running(&mut self) {
        self.status = NodeStatus::Running;
        self.started_at = Some(Utc::now());
    }

    /// Transition the node to Completed state.
    pub fn mark_completed(&mut self, duration_ms: u64) {
        self.status = NodeStatus::Completed;
        self.last_duration_ms = Some(duration_ms);
        self.total_duration_ms += duration_ms;
        self.completed_at = Some(Utc::now());
    }

    /// Transition the node to Failed state.
    pub fn mark_failed(&mut self, failure_type: String, error_message: String) {
        self.status = NodeStatus::Failed;
        self.last_failure_type = Some(failure_type);
        self.last_error = Some(error_message);
        self.completed_at = Some(Utc::now());
    }

    /// Transition the node to Skipped state.
    pub fn mark_skipped(&mut self, reason: String) {
        self.status = NodeStatus::Skipped;
        self.last_error = Some(reason);
        self.completed_at = Some(Utc::now());
    }

    /// Record a retry (increments attempt counter, resets to Ready).
    pub fn mark_for_retry(&mut self) {
        self.retry_attempts += 1;
        self.status = NodeStatus::Ready;
        self.last_duration_ms = None;
        self.started_at = None;
    }

    /// Returns true if the node is in a terminal state.
    pub fn is_terminal(&self) -> bool {
        self.status.is_terminal()
    }
}

// ---------------------------------------------------------------------------
// TaskResult — Result of a Single Node Execution
// ---------------------------------------------------------------------------

/// Result of executing a single node (task).
///
/// Carries the output, metadata, and outcome of a node execution.
/// Used by the parallel executor to collect results for aggregation.
///
/// # Contract (Frozen)
/// - Each node produces exactly one TaskResult per execution attempt
/// - Only the final attempt's result is preserved in ExecutionResult
/// - Output is optional — nodes may produce no output
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResult {
    /// The node ID that produced this result.
    pub node_id: Uuid,
    /// The node's name for display purposes.
    pub node_name: String,
    /// Whether the execution was successful.
    pub success: bool,
    /// Optional output produced by the node.
    pub output: Option<String>,
    /// Duration of the execution in milliseconds.
    pub duration_ms: u64,
    /// Number of retry attempts made (0 = first attempt success).
    pub retry_attempts: u8,
    /// Error message if execution failed.
    pub error: Option<String>,
    /// The failure type if execution failed.
    pub failure_type: Option<String>,
    /// ISO 8601 timestamp when execution started.
    pub started_at: DateTime<Utc>,
    /// ISO 8601 timestamp when execution completed.
    pub completed_at: DateTime<Utc>,
}

impl TaskResult {
    /// Create a new successful TaskResult.
    pub fn success(
        node_id: Uuid,
        node_name: impl Into<String>,
        output: Option<String>,
        duration_ms: u64,
        retry_attempts: u8,
    ) -> Self {
        let now = Utc::now();
        Self {
            node_id,
            node_name: node_name.into(),
            success: true,
            output,
            duration_ms,
            retry_attempts,
            error: None,
            failure_type: None,
            started_at: now,
            completed_at: now,
        }
    }

    /// Create a new failed TaskResult.
    pub fn failure(
        node_id: Uuid,
        node_name: impl Into<String>,
        error: String,
        failure_type: String,
        duration_ms: u64,
        retry_attempts: u8,
    ) -> Self {
        let now = Utc::now();
        Self {
            node_id,
            node_name: node_name.into(),
            success: false,
            output: None,
            duration_ms,
            retry_attempts,
            error: Some(error),
            failure_type: Some(failure_type),
            started_at: now,
            completed_at: now,
        }
    }
}

// ---------------------------------------------------------------------------
// ExecutionResult — Aggregate Result of a Full DAG Execution
// ---------------------------------------------------------------------------

/// Aggregate result of executing a full DAG through the parallel executor.
///
/// Collects all node results and provides summary statistics about the
/// execution run.
///
/// # Contract (Frozen)
/// - Contains the results for every node in the graph
/// - Provides summary counts by status
/// - Timestamps for execution lifecycle
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    /// The ID of the DAG that was executed.
    pub dag_id: Uuid,
    /// Results for each node, keyed by node ID.
    pub node_results: HashMap<Uuid, TaskResult>,
    /// Execution state for each node, keyed by node ID.
    pub execution_states: HashMap<Uuid, NodeExecutionState>,
    /// Number of nodes completed successfully.
    pub completed_count: u32,
    /// Number of nodes that failed permanently.
    pub failed_count: u32,
    /// Number of nodes skipped.
    pub skipped_count: u32,
    /// Total number of nodes in the graph.
    pub total_nodes: u32,
    /// Total execution duration in milliseconds.
    pub total_duration_ms: u64,
    /// Total number of retries across all nodes.
    pub total_retries: u32,
    /// ISO 8601 timestamp when execution started.
    pub started_at: DateTime<Utc>,
    /// ISO 8601 timestamp when execution completed.
    pub completed_at: DateTime<Utc>,
    /// Whether the execution was cancelled.
    pub cancelled: bool,
    /// Why the execution was cancelled (if applicable).
    pub cancellation_reason: Option<String>,
}

impl ExecutionResult {
    /// Create a new empty ExecutionResult.
    pub fn new(dag_id: Uuid) -> Self {
        Self {
            dag_id,
            node_results: HashMap::new(),
            execution_states: HashMap::new(),
            completed_count: 0,
            failed_count: 0,
            skipped_count: 0,
            total_nodes: 0,
            total_duration_ms: 0,
            total_retries: 0,
            started_at: Utc::now(),
            completed_at: Utc::now(),
            cancelled: false,
            cancellation_reason: None,
        }
    }

    /// Record a node result and update summary counts.
    pub fn record_result(&mut self, result: TaskResult) {
        if result.success {
            self.completed_count += 1;
        } else {
            self.failed_count += 1;
        }
        self.total_retries += result.retry_attempts as u32;
        self.total_duration_ms += result.duration_ms;
        self.node_results.insert(result.node_id, result);
    }

    /// Record a skipped node.
    pub fn record_skipped(&mut self, _node_id: Uuid) {
        self.skipped_count += 1;
    }

    /// Returns true if all nodes completed successfully.
    pub fn all_succeeded(&self) -> bool {
        self.failed_count == 0
            && self.skipped_count == 0
            && self.completed_count == self.total_nodes
    }

    /// Returns true if any nodes failed permanently.
    pub fn has_failures(&self) -> bool {
        self.failed_count > 0
    }

    /// Returns true if the execution had any unsuccessful outcomes.
    pub fn has_issues(&self) -> bool {
        self.failed_count > 0 || self.skipped_count > 0 || self.cancelled
    }
}

impl std::fmt::Display for ExecutionResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "ExecutionResult[dag={}, completed={}, failed={}, skipped={}, cancelled={}, duration={}ms]",
            self.dag_id,
            self.completed_count,
            self.failed_count,
            self.skipped_count,
            self.cancelled,
            self.total_duration_ms,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn test_node_status_variants() {
        assert_eq!(NodeStatus::Pending as u8, 0);
        assert_eq!(NodeStatus::Ready as u8, 1);
        assert_eq!(NodeStatus::Running as u8, 2);
        assert_eq!(NodeStatus::Completed as u8, 3);
        assert_eq!(NodeStatus::Failed as u8, 4);
        assert_eq!(NodeStatus::Skipped as u8, 5);
    }

    #[test]
    fn test_node_status_is_terminal() {
        assert!(!NodeStatus::Pending.is_terminal());
        assert!(!NodeStatus::Ready.is_terminal());
        assert!(!NodeStatus::Running.is_terminal());
        assert!(NodeStatus::Completed.is_terminal());
        assert!(NodeStatus::Failed.is_terminal());
        assert!(NodeStatus::Skipped.is_terminal());
    }

    #[test]
    fn test_node_status_can_execute() {
        assert!(!NodeStatus::Pending.can_execute());
        assert!(NodeStatus::Ready.can_execute());
        assert!(!NodeStatus::Running.can_execute());
        assert!(!NodeStatus::Completed.can_execute());
        assert!(!NodeStatus::Failed.can_execute());
        assert!(!NodeStatus::Skipped.can_execute());
    }

    #[test]
    fn test_node_status_as_str() {
        assert_eq!(NodeStatus::Pending.as_str(), "pending");
        assert_eq!(NodeStatus::Ready.as_str(), "ready");
        assert_eq!(NodeStatus::Running.as_str(), "running");
        assert_eq!(NodeStatus::Completed.as_str(), "completed");
        assert_eq!(NodeStatus::Failed.as_str(), "failed");
        assert_eq!(NodeStatus::Skipped.as_str(), "skipped");
    }

    #[test]
    fn test_task_result_success() {
        let node_id = Uuid::new_v4();
        let result = TaskResult::success(node_id, "test-node", Some("output".into()), 100, 0);
        assert!(result.success);
        assert_eq!(result.node_id, node_id);
        assert_eq!(result.node_name, "test-node");
        assert_eq!(result.output, Some("output".into()));
        assert_eq!(result.duration_ms, 100);
        assert_eq!(result.retry_attempts, 0);
        assert!(result.error.is_none());
    }

    #[test]
    fn test_task_result_failure() {
        let node_id = Uuid::new_v4();
        let result = TaskResult::failure(
            node_id,
            "test-node",
            "error msg".into(),
            "Transient".into(),
            50,
            2,
        );
        assert!(!result.success);
        assert_eq!(result.node_id, node_id);
        assert_eq!(result.error, Some("error msg".into()));
        assert_eq!(result.failure_type, Some("Transient".into()));
        assert_eq!(result.duration_ms, 50);
        assert_eq!(result.retry_attempts, 2);
    }

    #[test]
    fn test_execution_result_new() {
        let dag_id = Uuid::new_v4();
        let result = ExecutionResult::new(dag_id);
        assert_eq!(result.dag_id, dag_id);
        assert_eq!(result.completed_count, 0);
        assert_eq!(result.failed_count, 0);
        assert_eq!(result.skipped_count, 0);
        assert_eq!(result.total_nodes, 0);
        assert!(!result.cancelled);
        assert!(result.cancellation_reason.is_none());
        assert!(!result.has_failures());
        assert!(!result.has_issues());
    }

    #[test]
    fn test_execution_result_display() {
        let dag_id = Uuid::new_v4();
        let result = ExecutionResult::new(dag_id);
        let display = format!("{}", result);
        assert!(display.contains("completed=0"));
        assert!(display.contains("failed=0"));
        assert!(display.contains("dag="));
    }
}
