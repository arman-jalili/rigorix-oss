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
