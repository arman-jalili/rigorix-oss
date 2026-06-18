//! ExecutionEvent domain enum — all 11 variants + PersistedEvent.
//!
//! @canonical .pi/architecture/modules/event-system.md#events
//! Implements: Contract Freeze — ExecutionEvent tagged union with 11 variants
//! Issue: #46
//!
//! This is the central event type for the entire execution lifecycle.
//! Every phase — planning, node execution, tool calls, completion — emits
//! an `ExecutionEvent` variant through the `EventBus`.
//!
//! # Contract (Frozen)
//! - Exactly 11 variants, no more, no less
//! - Every variant carries `execution_id` for correlation
//! - Every variant carries `timestamp` (ISO 8601 UTC)
//! - Serialized as tagged union with `#[serde(tag = "type", rename_all = "snake_case")]`
//! - No implementation logic — pure data

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// All possible execution events emitted during a Rigorix run.
///
/// # Variants
///
/// | # | Variant | Emitted By | Purpose |
/// |---|---------|------------|---------|
/// | 1 | `PlanningStarted` | Orchestrator | Plan generation begins |
/// | 2 | `PlanningCompleted` | Orchestrator | Plan generated successfully |
/// | 3 | `NodeStarted` | ParallelExecutor | A DAG node begins execution |
/// | 4 | `NodeCompleted` | ParallelExecutor | A DAG node finishes successfully |
/// | 5 | `NodeFailed` | ParallelExecutor | A DAG node fails (may retry) |
/// | 6 | `NodeRetrying` | ParallelExecutor | A failed node is being retried |
/// | 7 | `ToolExecuted` | ExecutionEnforcer | A tool was called (allowed or skipped) |
/// | 8 | `ExecutionCompleted` | Orchestrator | Entire execution finished successfully |
/// | 9 | `ExecutionFailed` | Orchestrator | Execution terminated with error |
/// | 10 | `ExecutionCancelled` | Orchestrator | Execution was cancelled (user/graceful) |
/// | 11 | `BudgetWarning` | ExecutionEnforcer | A resource budget threshold was hit |
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ExecutionEvent {
    /// Execution plan generation has started.
    ///
    /// Emitted when the orchestrator begins generating the execution plan
    /// from the selected template and user intent.
    PlanningStarted {
        /// Globally unique execution identifier.
        execution_id: uuid::Uuid,
        /// The user's high-level intent / prompt.
        intent: String,
        /// ISO 8601 timestamp of when planning started.
        timestamp: DateTime<Utc>,
    },

    /// Execution plan generation completed successfully.
    ///
    /// Emitted when a valid execution plan has been produced.
    /// Carries the template ID, confidence score, and resolved parameters.
    PlanningCompleted {
        /// Globally unique execution identifier.
        execution_id: uuid::Uuid,
        /// The template that was selected for this execution.
        template_id: String,
        /// Model confidence score (0.0–1.0) for the generated plan.
        confidence: f64,
        /// Key-value pairs of resolved template parameters.
        parameters: HashMap<String, String>,
        /// ISO 8601 timestamp of when planning completed.
        timestamp: DateTime<Utc>,
    },

    /// A DAG node has started execution.
    ///
    /// Emitted before the node's action (tool call, sub-pipeline, script) begins.
    NodeStarted {
        /// Globally unique execution identifier.
        execution_id: uuid::Uuid,
        /// Identifier of the DAG node being executed.
        node_id: String,
        /// Human-readable name of the node.
        node_name: String,
        /// ISO 8601 timestamp of when the node started.
        timestamp: DateTime<Utc>,
    },

    /// A DAG node completed successfully.
    ///
    /// Emitted when the node's action returns without error.
    NodeCompleted {
        /// Globally unique execution identifier.
        execution_id: uuid::Uuid,
        /// Identifier of the DAG node that completed.
        node_id: String,
        /// Human-readable name of the node.
        node_name: String,
        /// Duration of node execution in milliseconds.
        duration_ms: u64,
        /// Structured output produced by this node.
        output: serde_json::Value,
        /// ISO 8601 timestamp of when the node completed.
        timestamp: DateTime<Utc>,
    },

    /// A DAG node failed during execution.
    ///
    /// Emitted when the node's action throws an error.
    /// May be followed by `NodeRetrying` if retries are configured.
    NodeFailed {
        /// Globally unique execution identifier.
        execution_id: uuid::Uuid,
        /// Identifier of the DAG node that failed.
        node_id: String,
        /// Error description.
        error: String,
        /// Which attempt this was (1-indexed).
        attempt: u32,
        /// ISO 8601 timestamp of when the failure occurred.
        timestamp: DateTime<Utc>,
    },

    /// A failed node is being retried.
    ///
    /// Emitted when the executor applies the configured retry strategy
    /// and schedules a re-run of the failed node.
    NodeRetrying {
        /// Globally unique execution identifier.
        execution_id: uuid::Uuid,
        /// Identifier of the DAG node being retried.
        node_id: String,
        /// Which retry attempt this is (1-indexed).
        attempt: u32,
        /// Delay before this retry in milliseconds.
        delay_ms: u64,
        /// ISO 8601 timestamp of when the retry was scheduled.
        timestamp: DateTime<Utc>,
    },

    /// A tool was executed (or skipped).
    ///
    /// Emitted by the ExecutionEnforcer every time a tool call is evaluated.
    /// `skipped` indicates the tool was blocked by policy or budget limits.
    ToolExecuted {
        /// Globally unique execution identifier.
        execution_id: uuid::Uuid,
        /// Identifier of the DAG node that requested the tool.
        node_id: String,
        /// Name of the tool being executed.
        tool: String,
        /// Risk level of the tool at time of execution.
        risk_level: String,
        /// Whether the tool was skipped (blocked by policy/budget).
        skipped: bool,
        /// ISO 8601 timestamp of tool execution.
        timestamp: DateTime<Utc>,
    },

    /// Execution completed successfully.
    ///
    /// Emitted when all DAG nodes have completed without unrecoverable errors.
    ExecutionCompleted {
        /// Globally unique execution identifier.
        execution_id: uuid::Uuid,
        /// Total execution duration in milliseconds.
        duration_ms: u64,
        /// Number of DAG nodes that were executed (not skipped).
        nodes_executed: u32,
        /// ISO 8601 timestamp of completion.
        timestamp: DateTime<Utc>,
    },

    /// Execution terminated with an unrecoverable error.
    ///
    /// Emitted when the execution cannot continue and is aborted.
    ExecutionFailed {
        /// Globally unique execution identifier.
        execution_id: uuid::Uuid,
        /// Final error detail describing why execution failed.
        error: String,
        /// ISO 8601 timestamp of the failure.
        timestamp: DateTime<Utc>,
    },

    /// Execution was cancelled (user-initiated or graceful shutdown).
    ///
    /// Emitted when the orchestrator receives a cancellation signal
    /// (e.g., Ctrl+C, shutdown request, graceful timeout) and terminates
    /// the execution before all nodes complete.
    ExecutionCancelled {
        /// Globally unique execution identifier.
        execution_id: uuid::Uuid,
        /// ISO 8601 timestamp of when cancellation was processed.
        timestamp: DateTime<Utc>,
    },

    /// A resource budget warning was triggered.
    ///
    /// Emitted by the ExecutionEnforcer when a tracked resource exceeds
    /// one of its budget thresholds (e.g., token usage, tool call count,
    /// execution time). Warnings are informational — the execution continues
    /// unless the hard limit is also hit.
    BudgetWarning {
        /// Globally unique execution identifier.
        execution_id: uuid::Uuid,
        /// The resource that exceeded its budget threshold
        /// (e.g., "tokens", "tool_calls", "execution_time_ms").
        resource: String,
        /// Current usage of the resource.
        used: u64,
        /// Budget limit for the resource.
        limit: u64,
        /// ISO 8601 timestamp of the warning.
        timestamp: DateTime<Utc>,
    },
}

/// A persisted execution event with a monotonic sequence number.
///
/// Wraps an `ExecutionEvent` with a globally unique, monotonically
/// increasing sequence number assigned at publish time. This enables
/// exact replay ordering and deduplication.
///
/// # Contract (Frozen)
/// - `sequence` is monotonically increasing within an `EventBus` instance
/// - `event` is the original `ExecutionEvent` with full payload
/// - Stored in `Vec<PersistedEvent>` inside `EventBus` for drain-at-end
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistedEvent {
    /// Monotonically increasing sequence number (1-indexed).
    pub sequence: u64,

    /// The original execution event.
    pub event: ExecutionEvent,
}

// ---------------------------------------------------------------------------
// Helper methods
// ---------------------------------------------------------------------------

impl ExecutionEvent {
    /// Returns the canonical snake_case name of this event variant.
    pub fn event_type_name(&self) -> &'static str {
        match self {
            ExecutionEvent::PlanningStarted { .. } => "planning_started",
            ExecutionEvent::PlanningCompleted { .. } => "planning_completed",
            ExecutionEvent::NodeStarted { .. } => "node_started",
            ExecutionEvent::NodeCompleted { .. } => "node_completed",
            ExecutionEvent::NodeFailed { .. } => "node_failed",
            ExecutionEvent::NodeRetrying { .. } => "node_retrying",
            ExecutionEvent::ToolExecuted { .. } => "tool_executed",
            ExecutionEvent::ExecutionCompleted { .. } => "execution_completed",
            ExecutionEvent::ExecutionFailed { .. } => "execution_failed",
            ExecutionEvent::ExecutionCancelled { .. } => "execution_cancelled",
            ExecutionEvent::BudgetWarning { .. } => "budget_warning",
        }
    }

    /// Returns the execution_id common to all variants.
    pub fn execution_id(&self) -> &uuid::Uuid {
        match self {
            ExecutionEvent::PlanningStarted { execution_id, .. }
            | ExecutionEvent::PlanningCompleted { execution_id, .. }
            | ExecutionEvent::NodeStarted { execution_id, .. }
            | ExecutionEvent::NodeCompleted { execution_id, .. }
            | ExecutionEvent::NodeFailed { execution_id, .. }
            | ExecutionEvent::NodeRetrying { execution_id, .. }
            | ExecutionEvent::ToolExecuted { execution_id, .. }
            | ExecutionEvent::ExecutionCompleted { execution_id, .. }
            | ExecutionEvent::ExecutionFailed { execution_id, .. }
            | ExecutionEvent::ExecutionCancelled { execution_id, .. }
            | ExecutionEvent::BudgetWarning { execution_id, .. } => execution_id,
        }
    }

    /// Returns the timestamp common to all variants.
    pub fn timestamp(&self) -> &DateTime<Utc> {
        match self {
            ExecutionEvent::PlanningStarted { timestamp, .. }
            | ExecutionEvent::PlanningCompleted { timestamp, .. }
            | ExecutionEvent::NodeStarted { timestamp, .. }
            | ExecutionEvent::NodeCompleted { timestamp, .. }
            | ExecutionEvent::NodeFailed { timestamp, .. }
            | ExecutionEvent::NodeRetrying { timestamp, .. }
            | ExecutionEvent::ToolExecuted { timestamp, .. }
            | ExecutionEvent::ExecutionCompleted { timestamp, .. }
            | ExecutionEvent::ExecutionFailed { timestamp, .. }
            | ExecutionEvent::ExecutionCancelled { timestamp, .. }
            | ExecutionEvent::BudgetWarning { timestamp, .. } => timestamp,
        }
    }

    /// Returns a human-friendly summary string for this event.
    pub fn summary(&self) -> String {
        match self {
            ExecutionEvent::PlanningStarted { intent, .. } => {
                format!(
                    "Planning started: {}",
                    intent.chars().take(80).collect::<String>()
                )
            }
            ExecutionEvent::PlanningCompleted {
                template_id,
                confidence,
                ..
            } => {
                format!(
                    "Planning completed: template={}, confidence={:.2}",
                    template_id, confidence
                )
            }
            ExecutionEvent::NodeStarted { node_name, .. } => {
                format!("Node started: {}", node_name)
            }
            ExecutionEvent::NodeCompleted {
                node_id,
                duration_ms,
                ..
            } => {
                format!("Node completed: {} ({}ms)", node_id, duration_ms)
            }
            ExecutionEvent::NodeFailed {
                node_id,
                error,
                attempt,
                ..
            } => {
                format!(
                    "Node failed: {} (attempt {}, error: {})",
                    node_id, attempt, error
                )
            }
            ExecutionEvent::NodeRetrying {
                node_id,
                attempt,
                delay_ms,
                ..
            } => {
                format!(
                    "Node retrying: {} (attempt {}, delay: {}ms)",
                    node_id, attempt, delay_ms
                )
            }
            ExecutionEvent::ToolExecuted {
                tool,
                risk_level,
                skipped,
                ..
            } => {
                if *skipped {
                    format!("Tool skipped: {} (risk: {})", tool, risk_level)
                } else {
                    format!("Tool executed: {} (risk: {})", tool, risk_level)
                }
            }
            ExecutionEvent::ExecutionCompleted { nodes_executed, .. } => {
                format!("Execution completed: {} nodes executed", nodes_executed)
            }
            ExecutionEvent::ExecutionFailed { error, .. } => {
                format!("Execution failed: {}", error)
            }
            ExecutionEvent::ExecutionCancelled { .. } => "Execution cancelled".to_string(),
            ExecutionEvent::BudgetWarning {
                resource,
                used,
                limit,
                ..
            } => {
                format!(
                    "Budget warning: {} used {}/{} (resource: {})",
                    resource, used, limit, resource
                )
            }
        }
    }

    /// Returns true if this variant represents a terminal state.
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            ExecutionEvent::ExecutionCompleted { .. }
                | ExecutionEvent::ExecutionFailed { .. }
                | ExecutionEvent::ExecutionCancelled { .. }
        )
    }

    /// Returns true if this variant represents an error/failure condition.
    pub fn is_error(&self) -> bool {
        matches!(
            self,
            ExecutionEvent::NodeFailed { .. }
                | ExecutionEvent::ExecutionFailed { .. }
                | ExecutionEvent::BudgetWarning { .. }
        )
    }
}

// ---------------------------------------------------------------------------
// Convenience constructors
// ---------------------------------------------------------------------------

impl ExecutionEvent {
    /// Create a PlanningStarted event.
    pub fn new_planning_started(execution_id: uuid::Uuid, intent: String) -> Self {
        ExecutionEvent::PlanningStarted {
            execution_id,
            intent,
            timestamp: Utc::now(),
        }
    }

    /// Create a PlanningCompleted event.
    pub fn new_planning_completed(
        execution_id: uuid::Uuid,
        template_id: String,
        confidence: f64,
        parameters: std::collections::HashMap<String, String>,
    ) -> Self {
        ExecutionEvent::PlanningCompleted {
            execution_id,
            template_id,
            confidence,
            parameters,
            timestamp: Utc::now(),
        }
    }

    /// Create a NodeStarted event.
    pub fn new_node_started(execution_id: uuid::Uuid, node_id: String, node_name: String) -> Self {
        ExecutionEvent::NodeStarted {
            execution_id,
            node_id,
            node_name,
            timestamp: Utc::now(),
        }
    }

    /// Create a NodeCompleted event.
    pub fn new_node_completed(
        execution_id: uuid::Uuid,
        node_id: String,
        node_name: String,
        duration_ms: u64,
        output: serde_json::Value,
    ) -> Self {
        ExecutionEvent::NodeCompleted {
            execution_id,
            node_id,
            node_name,
            duration_ms,
            output,
            timestamp: Utc::now(),
        }
    }

    /// Create a NodeFailed event.
    pub fn new_node_failed(
        execution_id: uuid::Uuid,
        node_id: String,
        error: String,
        attempt: u32,
    ) -> Self {
        ExecutionEvent::NodeFailed {
            execution_id,
            node_id,
            error,
            attempt,
            timestamp: Utc::now(),
        }
    }

    /// Create a NodeRetrying event.
    pub fn new_node_retrying(
        execution_id: uuid::Uuid,
        node_id: String,
        attempt: u32,
        delay_ms: u64,
    ) -> Self {
        ExecutionEvent::NodeRetrying {
            execution_id,
            node_id,
            attempt,
            delay_ms,
            timestamp: Utc::now(),
        }
    }

    /// Create a ToolExecuted event.
    pub fn new_tool_executed(
        execution_id: uuid::Uuid,
        node_id: String,
        tool: String,
        risk_level: String,
        skipped: bool,
    ) -> Self {
        ExecutionEvent::ToolExecuted {
            execution_id,
            node_id,
            tool,
            risk_level,
            skipped,
            timestamp: Utc::now(),
        }
    }

    /// Create an ExecutionCompleted event.
    pub fn new_execution_completed(
        execution_id: uuid::Uuid,
        duration_ms: u64,
        nodes_executed: u32,
    ) -> Self {
        ExecutionEvent::ExecutionCompleted {
            execution_id,
            duration_ms,
            nodes_executed,
            timestamp: Utc::now(),
        }
    }

    /// Create an ExecutionFailed event.
    pub fn new_execution_failed(execution_id: uuid::Uuid, error: String) -> Self {
        ExecutionEvent::ExecutionFailed {
            execution_id,
            error,
            timestamp: Utc::now(),
        }
    }

    /// Create an ExecutionCancelled event.
    pub fn new_execution_cancelled(execution_id: uuid::Uuid) -> Self {
        ExecutionEvent::ExecutionCancelled {
            execution_id,
            timestamp: Utc::now(),
        }
    }

    /// Create a BudgetWarning event.
    pub fn new_budget_warning(
        execution_id: uuid::Uuid,
        resource: String,
        used: u64,
        limit: u64,
    ) -> Self {
        ExecutionEvent::BudgetWarning {
            execution_id,
            resource,
            used,
            limit,
            timestamp: Utc::now(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn sample_eid() -> uuid::Uuid {
        uuid::Uuid::new_v4()
    }

    // -----------------------------------------------------------------------
    // Helper method tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_event_type_names() {
        let eid = sample_eid();
        assert_eq!(
            ExecutionEvent::new_planning_started(eid, "test".into()).event_type_name(),
            "planning_started"
        );
        assert_eq!(
            ExecutionEvent::new_planning_completed(eid, "t1".into(), 0.95, HashMap::new())
                .event_type_name(),
            "planning_completed"
        );
        assert_eq!(
            ExecutionEvent::new_node_started(eid, "n1".into(), "Node".into()).event_type_name(),
            "node_started"
        );
        assert_eq!(
            ExecutionEvent::new_node_completed(
                eid,
                "n1".into(),
                "Node 1".into(),
                100,
                serde_json::Value::Null
            )
            .event_type_name(),
            "node_completed"
        );
        assert_eq!(
            ExecutionEvent::new_node_failed(eid, "n1".into(), "err".into(), 1).event_type_name(),
            "node_failed"
        );
        assert_eq!(
            ExecutionEvent::new_node_retrying(eid, "n1".into(), 1, 100).event_type_name(),
            "node_retrying"
        );
        assert_eq!(
            ExecutionEvent::new_tool_executed(eid, "n1".into(), "bash".into(), "low".into(), false)
                .event_type_name(),
            "tool_executed"
        );
        assert_eq!(
            ExecutionEvent::new_execution_completed(eid, 1000, 5).event_type_name(),
            "execution_completed"
        );
        assert_eq!(
            ExecutionEvent::new_execution_failed(eid, "err".into()).event_type_name(),
            "execution_failed"
        );
        assert_eq!(
            ExecutionEvent::new_execution_cancelled(eid).event_type_name(),
            "execution_cancelled"
        );
        assert_eq!(
            ExecutionEvent::new_budget_warning(eid, "tokens".into(), 80, 100).event_type_name(),
            "budget_warning"
        );
    }

    #[test]
    fn test_execution_id_accessor() {
        let eid = sample_eid();
        let event = ExecutionEvent::new_planning_started(eid, "test".into());
        assert_eq!(*event.execution_id(), eid);

        let event = ExecutionEvent::new_execution_completed(eid, 100, 5);
        assert_eq!(*event.execution_id(), eid);
    }

    #[test]
    fn test_timestamp_accessor() {
        let eid = sample_eid();
        let event = ExecutionEvent::new_node_started(eid, "n1".into(), "Node".into());
        // Timestamp should be recent (within the last second)
        let elapsed = Utc::now() - *event.timestamp();
        assert!(elapsed.num_seconds() < 2);
    }

    #[test]
    fn test_summary_strings() {
        let eid = sample_eid();

        let event = ExecutionEvent::new_planning_started(eid, "Build the project".into());
        assert!(event.summary().contains("Planning started"));

        let event = ExecutionEvent::new_node_completed(
            eid,
            "compile".into(),
            "Compile".into(),
            500,
            serde_json::Value::Null,
        );
        assert!(event.summary().contains("compile"));
        assert!(event.summary().contains("500ms"));

        let event = ExecutionEvent::new_execution_failed(eid, "OOM".into());
        assert!(event.summary().contains("OOM"));
    }

    #[test]
    fn test_is_terminal() {
        let eid = sample_eid();
        assert!(ExecutionEvent::new_execution_completed(eid, 100, 5).is_terminal());
        assert!(ExecutionEvent::new_execution_failed(eid, "err".into()).is_terminal());
        assert!(ExecutionEvent::new_execution_cancelled(eid).is_terminal());
        assert!(!ExecutionEvent::new_node_started(eid, "n1".into(), "Node".into()).is_terminal());
        assert!(!ExecutionEvent::new_planning_started(eid, "test".into()).is_terminal());
    }

    #[test]
    fn test_is_error() {
        let eid = sample_eid();
        assert!(ExecutionEvent::new_node_failed(eid, "n1".into(), "err".into(), 1).is_error());
        assert!(ExecutionEvent::new_execution_failed(eid, "err".into()).is_error());
        assert!(ExecutionEvent::new_budget_warning(eid, "tokens".into(), 80, 100).is_error());
        assert!(
            !ExecutionEvent::new_node_completed(
                eid,
                "n1".into(),
                "Node 1".into(),
                100,
                serde_json::Value::Null
            )
            .is_error()
        );
        assert!(!ExecutionEvent::new_planning_started(eid, "test".into()).is_error());
    }

    // -----------------------------------------------------------------------
    // Constructor tests — all 11 variants
    // -----------------------------------------------------------------------

    #[test]
    fn test_new_planning_started() {
        let eid = sample_eid();
        let event = ExecutionEvent::new_planning_started(eid, "Generate plan".into());
        match &event {
            ExecutionEvent::PlanningStarted {
                execution_id,
                intent,
                ..
            } => {
                assert_eq!(*execution_id, eid);
                assert_eq!(intent, "Generate plan");
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_new_planning_completed() {
        let eid = sample_eid();
        let mut params = HashMap::new();
        params.insert("language".into(), "rust".into());
        let event = ExecutionEvent::new_planning_completed(eid, "build".into(), 0.95, params);
        match &event {
            ExecutionEvent::PlanningCompleted {
                execution_id,
                template_id,
                confidence,
                parameters,
                ..
            } => {
                assert_eq!(*execution_id, eid);
                assert_eq!(template_id, "build");
                assert!((*confidence - 0.95).abs() < 1e-6);
                assert_eq!(parameters.get("language").unwrap(), "rust");
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_new_node_started() {
        let eid = sample_eid();
        let event = ExecutionEvent::new_node_started(eid, "node-1".into(), "Compile".into());
        match &event {
            ExecutionEvent::NodeStarted {
                execution_id,
                node_id,
                node_name,
                ..
            } => {
                assert_eq!(*execution_id, eid);
                assert_eq!(node_id, "node-1");
                assert_eq!(node_name, "Compile");
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_new_node_completed() {
        let eid = sample_eid();
        let output = serde_json::json!({"status": "ok"});
        let event = ExecutionEvent::new_node_completed(
            eid,
            "node-1".into(),
            "Node 1".into(),
            250,
            output.clone(),
        );
        match &event {
            ExecutionEvent::NodeCompleted {
                execution_id,
                node_id,
                duration_ms,
                output: o,
                ..
            } => {
                assert_eq!(*execution_id, eid);
                assert_eq!(node_id, "node-1");
                assert_eq!(*duration_ms, 250);
                assert_eq!(*o, output);
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_new_node_failed() {
        let eid = sample_eid();
        let event =
            ExecutionEvent::new_node_failed(eid, "node-1".into(), "compilation error".into(), 2);
        match &event {
            ExecutionEvent::NodeFailed {
                execution_id,
                node_id,
                error,
                attempt,
                ..
            } => {
                assert_eq!(*execution_id, eid);
                assert_eq!(node_id, "node-1");
                assert_eq!(error, "compilation error");
                assert_eq!(*attempt, 2);
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_new_node_retrying() {
        let eid = sample_eid();
        let event = ExecutionEvent::new_node_retrying(eid, "node-1".into(), 2, 5000);
        match &event {
            ExecutionEvent::NodeRetrying {
                execution_id,
                node_id,
                attempt,
                delay_ms,
                ..
            } => {
                assert_eq!(*execution_id, eid);
                assert_eq!(node_id, "node-1");
                assert_eq!(*attempt, 2);
                assert_eq!(*delay_ms, 5000);
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_new_tool_executed() {
        let eid = sample_eid();
        let event = ExecutionEvent::new_tool_executed(
            eid,
            "node-1".into(),
            "bash".into(),
            "high".into(),
            false,
        );
        match &event {
            ExecutionEvent::ToolExecuted {
                execution_id,
                node_id,
                tool,
                risk_level,
                skipped,
                ..
            } => {
                assert_eq!(*execution_id, eid);
                assert_eq!(node_id, "node-1");
                assert_eq!(tool, "bash");
                assert_eq!(risk_level, "high");
                assert!(!skipped);
            }
            _ => panic!("Wrong variant"),
        }

        // Test skipped tool
        let event = ExecutionEvent::new_tool_executed(
            eid,
            "node-2".into(),
            "rm".into(),
            "critical".into(),
            true,
        );
        match &event {
            ExecutionEvent::ToolExecuted { skipped, .. } => assert!(*skipped),
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_new_execution_completed() {
        let eid = sample_eid();
        let event = ExecutionEvent::new_execution_completed(eid, 5000, 10);
        match &event {
            ExecutionEvent::ExecutionCompleted {
                execution_id,
                duration_ms,
                nodes_executed,
                ..
            } => {
                assert_eq!(*execution_id, eid);
                assert_eq!(*duration_ms, 5000);
                assert_eq!(*nodes_executed, 10);
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_new_execution_failed() {
        let eid = sample_eid();
        let event = ExecutionEvent::new_execution_failed(eid, "out of memory".into());
        match &event {
            ExecutionEvent::ExecutionFailed {
                execution_id,
                error,
                ..
            } => {
                assert_eq!(*execution_id, eid);
                assert_eq!(error, "out of memory");
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_new_execution_cancelled() {
        let eid = sample_eid();
        let event = ExecutionEvent::new_execution_cancelled(eid);
        match &event {
            ExecutionEvent::ExecutionCancelled { execution_id, .. } => {
                assert_eq!(*execution_id, eid);
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_new_budget_warning() {
        let eid = sample_eid();
        let event = ExecutionEvent::new_budget_warning(eid, "tokens".into(), 800, 1000);
        match &event {
            ExecutionEvent::BudgetWarning {
                execution_id,
                resource,
                used,
                limit,
                ..
            } => {
                assert_eq!(*execution_id, eid);
                assert_eq!(resource, "tokens");
                assert_eq!(*used, 800);
                assert_eq!(*limit, 1000);
            }
            _ => panic!("Wrong variant"),
        }
    }

    // -----------------------------------------------------------------------
    // Serde round-trip tests for all 11 variants
    // -----------------------------------------------------------------------

    #[test]
    fn test_serde_roundtrip_planning_started() {
        let eid = sample_eid();
        let event = ExecutionEvent::new_planning_started(eid, "Generate deployment plan".into());
        let json = serde_json::to_string(&event).unwrap();
        let deserialized: ExecutionEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.event_type_name(), "planning_started");
        assert_eq!(*deserialized.execution_id(), eid);
    }

    #[test]
    fn test_serde_roundtrip_planning_completed() {
        let eid = sample_eid();
        let event = ExecutionEvent::new_planning_completed(eid, "t1".into(), 0.95, HashMap::new());
        let json = serde_json::to_string(&event).unwrap();
        let deserialized: ExecutionEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.event_type_name(), "planning_completed");
    }

    #[test]
    fn test_serde_roundtrip_node_started() {
        let eid = sample_eid();
        let event = ExecutionEvent::new_node_started(eid, "n1".into(), "Compile".into());
        let json = serde_json::to_string(&event).unwrap();
        let deserialized: ExecutionEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.event_type_name(), "node_started");
    }

    #[test]
    fn test_serde_roundtrip_node_completed() {
        let eid = sample_eid();
        let event = ExecutionEvent::new_node_completed(
            eid,
            "n1".into(),
            "Node 1".into(),
            250,
            serde_json::json!("done"),
        );
        let json = serde_json::to_string(&event).unwrap();
        let deserialized: ExecutionEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.event_type_name(), "node_completed");
    }

    #[test]
    fn test_serde_roundtrip_node_failed() {
        let eid = sample_eid();
        let event = ExecutionEvent::new_node_failed(eid, "n1".into(), "error".into(), 1);
        let json = serde_json::to_string(&event).unwrap();
        let deserialized: ExecutionEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.event_type_name(), "node_failed");
    }

    #[test]
    fn test_serde_roundtrip_node_retrying() {
        let eid = sample_eid();
        let event = ExecutionEvent::new_node_retrying(eid, "n1".into(), 2, 5000);
        let json = serde_json::to_string(&event).unwrap();
        let deserialized: ExecutionEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.event_type_name(), "node_retrying");
    }

    #[test]
    fn test_serde_roundtrip_tool_executed() {
        let eid = sample_eid();
        let event =
            ExecutionEvent::new_tool_executed(eid, "n1".into(), "bash".into(), "low".into(), false);
        let json = serde_json::to_string(&event).unwrap();
        let deserialized: ExecutionEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.event_type_name(), "tool_executed");

        // Also test skipped=true round-trip
        let event_skipped = ExecutionEvent::new_tool_executed(
            eid,
            "n1".into(),
            "rm".into(),
            "critical".into(),
            true,
        );
        let json = serde_json::to_string(&event_skipped).unwrap();
        let deserialized: ExecutionEvent = serde_json::from_str(&json).unwrap();
        match &deserialized {
            ExecutionEvent::ToolExecuted { skipped, .. } => assert!(*skipped),
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_serde_roundtrip_execution_completed() {
        let eid = sample_eid();
        let event = ExecutionEvent::new_execution_completed(eid, 5000, 10);
        let json = serde_json::to_string(&event).unwrap();
        let deserialized: ExecutionEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.event_type_name(), "execution_completed");
    }

    #[test]
    fn test_serde_roundtrip_execution_failed() {
        let eid = sample_eid();
        let event = ExecutionEvent::new_execution_failed(eid, "crash".into());
        let json = serde_json::to_string(&event).unwrap();
        let deserialized: ExecutionEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.event_type_name(), "execution_failed");
    }

    #[test]
    fn test_serde_roundtrip_execution_cancelled() {
        let eid = sample_eid();
        let event = ExecutionEvent::new_execution_cancelled(eid);
        let json = serde_json::to_string(&event).unwrap();
        let deserialized: ExecutionEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.event_type_name(), "execution_cancelled");
    }

    #[test]
    fn test_serde_roundtrip_budget_warning() {
        let eid = sample_eid();
        let event = ExecutionEvent::new_budget_warning(eid, "tokens".into(), 800, 1000);
        let json = serde_json::to_string(&event).unwrap();
        let deserialized: ExecutionEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.event_type_name(), "budget_warning");
    }

    #[test]
    fn test_serde_tagged_union_format() {
        // Verify the JSON format uses "type" tag with snake_case
        let eid = sample_eid();
        let event = ExecutionEvent::new_node_started(eid, "n1".into(), "Build".into());
        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(
            json.get("type").and_then(|v| v.as_str()),
            Some("node_started")
        );

        let event = ExecutionEvent::new_execution_cancelled(eid);
        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(
            json.get("type").and_then(|v| v.as_str()),
            Some("execution_cancelled")
        );
    }

    #[test]
    fn test_serde_unknown_variant_fails() {
        // Deserializing an unknown variant should error
        let bad_json = r#"{"type":"unknown_variant","execution_id":"00000000-0000-0000-0000-000000000000","timestamp":"2026-01-01T00:00:00Z"}"#;
        let result: Result<ExecutionEvent, _> = serde_json::from_str(bad_json);
        assert!(result.is_err());
    }

    // -----------------------------------------------------------------------
    // PersistedEvent tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_persisted_event_construction() {
        let eid = sample_eid();
        let event = ExecutionEvent::new_node_started(eid, "n1".into(), "Build".into());
        let persisted = PersistedEvent {
            sequence: 42,
            event: event.clone(),
        };
        assert_eq!(persisted.sequence, 42);
        assert_eq!(*persisted.event.execution_id(), eid);
    }

    #[test]
    fn test_persisted_event_serde_roundtrip() {
        let eid = sample_eid();
        let persisted = PersistedEvent {
            sequence: 1,
            event: ExecutionEvent::new_planning_started(eid, "plan".into()),
        };
        let json = serde_json::to_string(&persisted).unwrap();
        let deserialized: PersistedEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.sequence, 1);
        assert_eq!(*deserialized.event.execution_id(), eid);
    }

    // -----------------------------------------------------------------------
    // ExecutionEvent summary tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_summary_skipped_tool() {
        let eid = sample_eid();
        let event = ExecutionEvent::new_tool_executed(
            eid,
            "n1".into(),
            "rm".into(),
            "critical".into(),
            true,
        );
        assert!(event.summary().contains("skipped"));

        let event =
            ExecutionEvent::new_tool_executed(eid, "n1".into(), "ls".into(), "low".into(), false);
        assert!(!event.summary().contains("skipped"));
    }

    #[test]
    fn test_summary_intent_truncation() {
        let eid = sample_eid();
        let long_intent = "a".repeat(200);
        let event = ExecutionEvent::new_planning_started(eid, long_intent.clone());
        // Summary should truncate to 80 chars
        assert!(event.summary().len() < 200);
    }
}
