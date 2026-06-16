//! ExecutionRecord — Complete output of a Rigorix run.
//!
//! @canonical .pi/architecture/modules/orchestrator.md#record
//! Implements: Contract Freeze — ExecutionRecord aggregate
//! Issue: #338
//!
//! Complete output of a run, containing everything needed for audit, TUI,
//! and persistence. Built by the orchestrator after the full lifecycle.
//!
//! # Contract (Frozen)
//! - All fields are public for direct construction by the orchestrator
//! - Field types are domain primitives (no framework-specific types)
//! - The record is serializable for persistence and API responses
//! - Optional fields use `Option` to handle partial completion

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Complete output of a Rigorix execution run.
///
/// Aggregates planning metadata, per-node task results, the drained event log,
/// and execution timing information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionRecord {
    /// Globally unique execution identifier (UUIDv7).
    pub execution_id: uuid::Uuid,

    /// Metadata from the planning phase.
    pub planning: PlanningMetadata,

    /// Per-node task results from the DAG execution.
    pub task_results: Vec<TaskResult>,

    /// Drained event log from the EventBus.
    pub events: Vec<ExecutionEventInfo>,

    /// Execution context metadata.
    pub context: ExecutionContext,

    /// ISO 8601 timestamp when execution started.
    pub started_at: chrono::DateTime<chrono::Utc>,

    /// ISO 8601 timestamp when execution completed (None if failed before completion).
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,

    /// Total wall-clock duration in milliseconds.
    pub duration_ms: u64,

    /// Overall execution status.
    pub status: ExecutionStatus,
}

/// Metadata captured during the planning phase.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanningMetadata {
    /// Identifier of the template used for planning.
    pub template_id: String,

    /// Confidence score from the classifier (0.0–1.0).
    pub confidence: f64,

    /// Number of LLM calls made during planning.
    pub llm_calls: u32,

    /// Total tokens consumed across all LLM calls.
    pub total_tokens: u32,

    /// Hash of the planning prompt for replay reproducibility.
    pub prompt_hash: String,
}

/// Result of executing a single DAG node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResult {
    /// Unique identifier of the DAG node.
    pub node_id: String,

    /// Human-readable node name.
    pub node_name: String,

    /// Status of this task.
    pub status: TaskStatus,

    /// Duration in milliseconds.
    pub duration_ms: u64,

    /// Output produced by the task (if any).
    pub output: Option<String>,

    /// Error detail if the task failed.
    pub error: Option<String>,

    /// Number of retry attempts made.
    pub retry_attempts: u32,

    /// Tool used to execute the task (if applicable).
    pub tool_used: Option<String>,
}

/// A single execution event from the drained event log.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionEventInfo {
    /// Machine-readable event type.
    pub event_type: String,

    /// Human-readable event summary.
    pub summary: String,

    /// ISO 8601 timestamp.
    pub occurred_at: chrono::DateTime<chrono::Utc>,

    /// Correlation ID linking this event across services.
    pub correlation_id: Option<uuid::Uuid>,

    /// Event payload as JSON (if available).
    pub payload: Option<serde_json::Value>,

    /// Status of the event.
    pub status: EventInfoStatus,
}

/// Execution context metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionContext {
    /// Repository root path.
    pub repo_root: String,

    /// Hash of the symbol graph at execution time.
    pub symbol_graph_hash: Option<String>,

    /// Git commit hash (if available).
    pub git_commit: Option<String>,

    /// Git branch name (if available).
    pub git_branch: Option<String>,

    /// Execution environment (cli, ci, ide).
    pub environment: String,

    /// Arbitrary key-value metadata.
    pub metadata: HashMap<String, String>,
}

/// Overall execution status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecutionStatus {
    /// Execution completed successfully.
    Completed,
    /// Execution completed with some failures.
    PartialFailure,
    /// Execution failed entirely.
    Failed,
    /// Execution was cancelled.
    Cancelled,
}

/// Status of a single DAG task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskStatus {
    /// Task completed successfully.
    Success,
    /// Task failed.
    Failure,
    /// Task was skipped (e.g. conditional).
    Skipped,
    /// Task was cancelled.
    Cancelled,
    /// Task is still pending.
    Pending,
}

/// Status of an event in the drained log.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EventInfoStatus {
    /// Event represents a successful occurrence.
    Success,
    /// Event represents a failure.
    Failure,
    /// Event was informational.
    Info,
}

// ---------------------------------------------------------------------------
// Helper implementations
// ---------------------------------------------------------------------------

impl ExecutionRecord {
    /// Create a new execution record with the given execution ID.
    pub fn new(execution_id: uuid::Uuid, started_at: chrono::DateTime<chrono::Utc>) -> Self {
        Self {
            execution_id,
            planning: PlanningMetadata::default(),
            task_results: vec![],
            events: vec![],
            context: ExecutionContext::new(),
            started_at,
            completed_at: None,
            duration_ms: 0,
            status: ExecutionStatus::Completed,
        }
    }

    /// Return the total number of tasks in this execution.
    pub fn task_count(&self) -> usize {
        self.task_results.len()
    }

    /// Return the number of failed tasks.
    pub fn failed_count(&self) -> usize {
        self.task_results
            .iter()
            .filter(|t| t.status == TaskStatus::Failure)
            .count()
    }

    /// Return true if the execution completed with all tasks successful.
    pub fn is_success(&self) -> bool {
        self.status == ExecutionStatus::Completed
    }

    /// Return true if the execution was cancelled.
    pub fn is_cancelled(&self) -> bool {
        self.status == ExecutionStatus::Cancelled
    }

    /// Return true if the execution has any failures.
    pub fn has_failures(&self) -> bool {
        self.failed_count() > 0
    }

    /// Return a summary string for the record.
    pub fn summary(&self) -> String {
        format!(
            "execution={} status={:?} tasks={} failed={} duration={}ms",
            self.execution_id,
            self.status,
            self.task_count(),
            self.failed_count(),
            self.duration_ms,
        )
    }
}

impl Default for PlanningMetadata {
    fn default() -> Self {
        Self {
            template_id: String::new(),
            confidence: 0.0,
            llm_calls: 0,
            total_tokens: 0,
            prompt_hash: String::new(),
        }
    }
}

impl ExecutionContext {
    /// Create a new empty execution context.
    pub fn new() -> Self {
        Self {
            repo_root: String::new(),
            symbol_graph_hash: None,
            git_commit: None,
            git_branch: None,
            environment: "cli".to_string(),
            metadata: HashMap::new(),
        }
    }
}

impl Default for ExecutionContext {
    fn default() -> Self {
        Self::new()
    }
}

impl TaskResult {
    /// Create a new successful task result.
    pub fn success(node_id: String, node_name: String, duration_ms: u64) -> Self {
        Self {
            node_id,
            node_name,
            status: TaskStatus::Success,
            duration_ms,
            output: None,
            error: None,
            retry_attempts: 0,
            tool_used: None,
        }
    }

    /// Create a new failed task result.
    pub fn failure(node_id: String, node_name: String, error: String, duration_ms: u64) -> Self {
        Self {
            node_id,
            node_name,
            status: TaskStatus::Failure,
            duration_ms,
            output: None,
            error: Some(error),
            retry_attempts: 0,
            tool_used: None,
        }
    }
}

impl TaskStatus {
    /// Return true if this status represents a terminal state.
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            TaskStatus::Success | TaskStatus::Failure | TaskStatus::Skipped | TaskStatus::Cancelled
        )
    }
}

impl ExecutionStatus {
    /// Return true if this status represents a terminal state.
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            ExecutionStatus::Completed
                | ExecutionStatus::PartialFailure
                | ExecutionStatus::Failed
                | ExecutionStatus::Cancelled
        )
    }

    /// Return true if this status represents an error condition.
    pub fn is_error(&self) -> bool {
        matches!(
            self,
            ExecutionStatus::PartialFailure | ExecutionStatus::Failed
        )
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_execution_record_new() {
        let id = uuid::Uuid::new_v4();
        let now = chrono::Utc::now();
        let record = ExecutionRecord::new(id, now);
        assert_eq!(record.execution_id, id);
        assert_eq!(record.status, ExecutionStatus::Completed);
        assert_eq!(record.task_count(), 0);
        assert_eq!(record.failed_count(), 0);
        assert!(record.is_success());
        assert!(!record.is_cancelled());
        assert!(!record.has_failures());
    }

    #[test]
    fn test_execution_record_summary() {
        let id = uuid::Uuid::new_v4();
        let now = chrono::Utc::now();
        let record = ExecutionRecord::new(id, now);
        let summary = record.summary();
        assert!(summary.contains(&id.to_string()));
        assert!(summary.contains("Completed"));
        assert!(summary.contains("tasks=0"));
    }

    #[test]
    fn test_task_result_success() {
        let task = TaskResult::success("1".into(), "task1".into(), 100);
        assert_eq!(task.status, TaskStatus::Success);
        assert_eq!(task.node_id, "1");
        assert_eq!(task.duration_ms, 100);
        assert!(task.error.is_none());
    }

    #[test]
    fn test_task_result_failure() {
        let task = TaskResult::failure("1".into(), "task1".into(), "error!".into(), 50);
        assert_eq!(task.status, TaskStatus::Failure);
        assert_eq!(task.error, Some("error!".to_string()));
    }

    #[test]
    fn test_task_status_is_terminal() {
        assert!(TaskStatus::Success.is_terminal());
        assert!(TaskStatus::Failure.is_terminal());
        assert!(TaskStatus::Skipped.is_terminal());
        assert!(TaskStatus::Cancelled.is_terminal());
        assert!(!TaskStatus::Pending.is_terminal());
    }

    #[test]
    fn test_execution_status_is_terminal() {
        assert!(ExecutionStatus::Completed.is_terminal());
        assert!(ExecutionStatus::PartialFailure.is_terminal());
        assert!(ExecutionStatus::Failed.is_terminal());
        assert!(ExecutionStatus::Cancelled.is_terminal());
    }

    #[test]
    fn test_execution_status_is_error() {
        assert!(ExecutionStatus::PartialFailure.is_error());
        assert!(ExecutionStatus::Failed.is_error());
        assert!(!ExecutionStatus::Completed.is_error());
        assert!(!ExecutionStatus::Cancelled.is_error());
    }

    #[test]
    fn test_execution_context_default() {
        let ctx = ExecutionContext::new();
        assert_eq!(ctx.repo_root, "");
        assert_eq!(ctx.environment, "cli");
        assert!(ctx.metadata.is_empty());
        assert!(ctx.symbol_graph_hash.is_none());
    }

    #[test]
    fn test_planning_metadata_default() {
        let pm = PlanningMetadata::default();
        assert_eq!(pm.template_id, "");
        assert_eq!(pm.confidence, 0.0);
        assert_eq!(pm.llm_calls, 0);
    }

    #[test]
    fn test_execution_record_serialization_roundtrip() {
        let id = uuid::Uuid::new_v4();
        let now = chrono::Utc::now();
        let mut record = ExecutionRecord::new(id, now);
        record.duration_ms = 1500;
        record.status = ExecutionStatus::PartialFailure;
        record.task_results.push(TaskResult::success("1".into(), "t1".into(), 100));
        record.task_results.push(TaskResult::failure("2".into(), "t2".into(), "err".into(), 50));

        let json = serde_json::to_string(&record).unwrap();
        let deserialized: ExecutionRecord = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.execution_id, id);
        assert_eq!(deserialized.status, ExecutionStatus::PartialFailure);
        assert_eq!(deserialized.task_count(), 2);
        assert_eq!(deserialized.failed_count(), 1);
        assert!(deserialized.has_failures());
        assert!(!deserialized.is_success());
        assert_eq!(deserialized.duration_ms, 1500);
    }

    #[test]
    fn test_execution_record_event_info() {
        let info = ExecutionEventInfo {
            event_type: "node_completed".into(),
            summary: "Node completed".into(),
            occurred_at: chrono::Utc::now(),
            correlation_id: None,
            payload: Some(serde_json::json!({ "key": "value" })),
            status: EventInfoStatus::Success,
        };
        assert_eq!(info.event_type, "node_completed");
        assert!(info.payload.is_some());
        assert_eq!(info.status, EventInfoStatus::Success);
    }

    #[test]
    fn test_event_info_status_variants() {
        assert_eq!(format!("{:?}", EventInfoStatus::Success), "Success");
        assert_eq!(format!("{:?}", EventInfoStatus::Failure), "Failure");
        assert_eq!(format!("{:?}", EventInfoStatus::Info), "Info");
    }
}
