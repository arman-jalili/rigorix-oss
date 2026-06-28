//! Event payload schemas for the Execution Engine bounded context.
//!
//! @canonical .pi/architecture/modules/execution-engine.md#events
//! Implements: Contract Freeze — ExecutionEvent payload schemas
//! Issue: issue-contract-freeze
//!
//! These events are emitted on the `EventBus` whenever significant execution
//! lifecycle events occur — node started, node completed, node failed, retry
//! triggered, fallback executed, execution completed.
//!
//! # Contract (Frozen)
//! - Each event carries the full context needed by consumers
//! - No internal implementation details exposed
//! - `dag_id` correlates to the originating TaskGraph
//! - `node_id` correlates to the originating TaskNode

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Events emitted by the Execution Engine module.
///
/// Wrapped in `CoreOrchestratorEvent::execution_event(...)` at the
/// orchestration layer for unified event bus emission.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ExecutionEngineEvent {
    /// A DAG execution session has started.
    ///
    /// Emitted when `ParallelExecutor::execute_graph()` is called
    /// and the ready queue is populated.
    ExecutionStarted {
        /// Globally unique identifier for this DAG execution.
        dag_id: Uuid,
        /// Total number of nodes to execute.
        total_nodes: u32,
        /// ISO 8601 timestamp of start.
        timestamp: DateTime<Utc>,
    },

    /// A node has started execution.
    ///
    /// Emitted when a task is dispatched to the executor for processing.
    NodeExecutionStarted {
        /// Globally unique identifier for this DAG execution.
        dag_id: Uuid,
        /// The node ID that started execution.
        node_id: Uuid,
        /// The node's name for display purposes.
        node_name: String,
        /// The attempt number (0 = first attempt, 1+ = retry).
        attempt: u8,
        /// ISO 8601 timestamp of start.
        timestamp: DateTime<Utc>,
    },

    /// A node has completed execution successfully.
    ///
    /// Emitted when a node finishes execution and all post-execution
    /// checks pass.
    NodeExecutionCompleted {
        /// Globally unique identifier for this DAG execution.
        dag_id: Uuid,
        /// The node ID that completed.
        node_id: Uuid,
        /// The node's name for display purposes.
        node_name: String,
        /// Duration of execution in milliseconds.
        duration_ms: u64,
        /// ISO 8601 timestamp of completion.
        timestamp: DateTime<Utc>,
    },

    /// A node has failed execution.
    ///
    /// Emitted when a node fails. The executor will check the retry
    /// policy to decide whether to retry, fallback, or propagate the error.
    NodeExecutionFailed {
        /// Globally unique identifier for this DAG execution.
        dag_id: Uuid,
        /// The node ID that failed.
        node_id: Uuid,
        /// The node's name for display purposes.
        node_name: String,
        /// The failure type classification.
        failure_type: String,
        /// The error message from the failure.
        error_message: String,
        /// Number of retries remaining before permanent failure.
        retries_remaining: u8,
        /// ISO 8601 timestamp of failure.
        timestamp: DateTime<Utc>,
    },

    /// A node is being retried after failure.
    ///
    /// Emitted when the retry policy determines the node should be
    /// retried with a specific strategy and backoff delay.
    NodeRetried {
        /// Globally unique identifier for this DAG execution.
        dag_id: Uuid,
        /// The node ID being retried.
        node_id: Uuid,
        /// The node's name for display purposes.
        node_name: String,
        /// The retry attempt number (1-indexed).
        attempt: u8,
        /// The retry strategy being applied.
        strategy: String,
        /// The computed backoff delay in milliseconds.
        backoff_ms: u64,
        /// ISO 8601 timestamp of retry.
        timestamp: DateTime<Utc>,
    },

    /// A fallback node is being executed.
    ///
    /// Emitted when retry limits are exhausted and a configured fallback
    /// node is dispatched instead.
    FallbackExecuted {
        /// Globally unique identifier for this DAG execution.
        dag_id: Uuid,
        /// The original node ID that failed permanently.
        original_node_id: Uuid,
        /// The fallback node ID being executed.
        fallback_node_id: Uuid,
        /// The fallback node's name for display purposes.
        fallback_node_name: String,
        /// ISO 8601 timestamp of fallback execution.
        timestamp: DateTime<Utc>,
    },

    /// A node's execution has been skipped via SkipAndContinue strategy.
    NodeSkipped {
        /// Globally unique identifier for this DAG execution.
        dag_id: Uuid,
        /// The node ID that was skipped.
        node_id: Uuid,
        /// The node's name for display purposes.
        node_name: String,
        /// Reason for skipping.
        reason: String,
        /// ISO 8601 timestamp of skip.
        timestamp: DateTime<Utc>,
    },

    /// A dependency resolution conflict occurred.
    DependencyResolutionConflict {
        /// Globally unique identifier for this DAG execution.
        dag_id: Uuid,
        /// The node ID that encountered the conflict.
        node_id: Uuid,
        /// The node's name for display purposes.
        node_name: String,
        /// List of dependency IDs that were unsatisfied.
        unsatisfied_deps: Vec<Uuid>,
        /// ISO 8601 timestamp of the conflict.
        timestamp: DateTime<Utc>,
    },

    /// The DAG execution has completed (all nodes in terminal state).
    ///
    /// Emitted when all nodes in the graph have reached a terminal
    /// state (Completed, Failed, or Skipped).
    ExecutionCompleted {
        /// Globally unique identifier for this DAG execution.
        dag_id: Uuid,
        /// Total number of nodes in the graph.
        total_nodes: u32,
        /// Number of nodes completed successfully.
        completed_count: u32,
        /// Number of nodes that failed permanently.
        failed_count: u32,
        /// Number of nodes skipped.
        skipped_count: u32,
        /// Total execution duration in milliseconds.
        total_duration_ms: u64,
        /// ISO 8601 timestamp of completion.
        timestamp: DateTime<Utc>,
    },

    /// The execution was cancelled by a cancellation signal.
    ExecutionCancelled {
        /// Globally unique identifier for this DAG execution.
        dag_id: Uuid,
        /// Number of nodes completed before cancellation.
        completed_count: u32,
        /// Number of nodes that were waiting in the queue.
        remaining_count: u32,
        /// Human-readable reason for cancellation.
        reason: String,
        /// ISO 8601 timestamp of cancellation.
        timestamp: DateTime<Utc>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn test_execution_engine_event_execution_started() {
        let dag_id = Uuid::new_v4();
        let event = ExecutionEngineEvent::ExecutionStarted {
            dag_id,
            total_nodes: 5,
            timestamp: Utc::now(),
        };
        match event {
            ExecutionEngineEvent::ExecutionStarted {
                dag_id: id,
                total_nodes,
                ..
            } => {
                assert_eq!(id, dag_id);
                assert_eq!(total_nodes, 5);
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_execution_engine_event_node_execution_started() {
        let dag_id = Uuid::new_v4();
        let node_id = Uuid::new_v4();
        let event = ExecutionEngineEvent::NodeExecutionStarted {
            dag_id,
            node_id,
            node_name: "test-node".into(),
            attempt: 0,
            timestamp: Utc::now(),
        };
        match event {
            ExecutionEngineEvent::NodeExecutionStarted { node_name, .. } => {
                assert_eq!(node_name, "test-node");
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_execution_engine_event_node_completed() {
        let event = ExecutionEngineEvent::NodeExecutionCompleted {
            dag_id: Uuid::new_v4(),
            node_id: Uuid::new_v4(),
            node_name: "done".into(),
            duration_ms: 42,
            timestamp: Utc::now(),
        };
        match event {
            ExecutionEngineEvent::NodeExecutionCompleted {
                node_name,
                duration_ms,
                ..
            } => {
                assert_eq!(node_name, "done");
                assert_eq!(duration_ms, 42);
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_execution_engine_event_node_failed() {
        let event = ExecutionEngineEvent::NodeExecutionFailed {
            dag_id: Uuid::new_v4(),
            node_id: Uuid::new_v4(),
            node_name: "fail".into(),
            error_message: "timeout".into(),
            failure_type: "Transient".into(),
            retries_remaining: 3,
            timestamp: Utc::now(),
        };
        match event {
            ExecutionEngineEvent::NodeExecutionFailed { error_message, .. } => {
                assert_eq!(error_message, "timeout");
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_execution_engine_event_execution_completed() {
        let event = ExecutionEngineEvent::ExecutionCompleted {
            dag_id: Uuid::new_v4(),
            total_nodes: 10,
            completed_count: 8,
            failed_count: 1,
            skipped_count: 1,
            total_duration_ms: 1000,
            timestamp: Utc::now(),
        };
        match event {
            ExecutionEngineEvent::ExecutionCompleted {
                total_nodes,
                completed_count,
                ..
            } => {
                assert_eq!(total_nodes, 10);
                assert_eq!(completed_count, 8);
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_execution_engine_event_serialization() {
        let event = ExecutionEngineEvent::ExecutionStarted {
            dag_id: Uuid::new_v4(),
            total_nodes: 3,
            timestamp: Utc::now(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("execution_started"));
        let deserialized: ExecutionEngineEvent = serde_json::from_str(&json).unwrap();
        match deserialized {
            ExecutionEngineEvent::ExecutionStarted { total_nodes, .. } => {
                assert_eq!(total_nodes, 3);
            }
            _ => panic!("Wrong variant after deserialization"),
        }
    }
}
