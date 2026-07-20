//! ScoredEvaluationEvent — domain events for the scoring lifecycle.
//!
//! @canonical .pi/architecture/modules/scored-evaluation.md#scored-evaluation-event
//! Implements: Contract Freeze — ScoredEvaluationEvent enum
//! Issue: #673 (scored-evaluation epic)
//!
//! # Contract (Frozen)
//! - Three lifecycle events: Started, Completed, Failed
//! - Uses `node_id: String` to match existing `ExecutionEvent` convention
//! - Implements `Clone`, `Debug`, `PartialEq` for testability
//! - Serialization support for event bus and audit persistence

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::result::ScoringResult;

/// Domain events for the scored evaluation lifecycle.
///
/// These events are published on the EventBus whenever a scored evaluation
/// transitions between lifecycle states.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ScoredEvaluationEvent {
    /// A scored evaluation has started.
    ScoredEvaluationStarted {
        /// The node being evaluated (as String, matching ExecutionEvent convention).
        node_id: String,
        /// The execution this evaluation belongs to.
        execution_id: Uuid,
        /// Name of the backend performing the evaluation.
        backend: String,
        /// When the evaluation started.
        timestamp: DateTime<Utc>,
    },

    /// A scored evaluation completed successfully.
    ScoredEvaluationCompleted {
        /// The node that was evaluated.
        node_id: String,
        /// The execution this evaluation belongs to.
        execution_id: Uuid,
        /// The scoring result.
        result: ScoringResult,
        /// When the evaluation completed.
        timestamp: DateTime<Utc>,
    },

    /// A scored evaluation failed.
    ScoredEvaluationFailed {
        /// The node that failed.
        node_id: String,
        /// The execution this evaluation belongs to.
        execution_id: Uuid,
        /// Human-readable error description.
        error: String,
        /// When the evaluation failed.
        timestamp: DateTime<Utc>,
    },
}

impl ScoredEvaluationEvent {
    /// Returns a human-readable log line for this event.
    pub fn log_line(&self) -> String {
        match self {
            ScoredEvaluationEvent::ScoredEvaluationStarted {
                node_id, backend, ..
            } => {
                format!(
                    "[ScoredEvaluation] Started: node={}, backend={}",
                    node_id, backend
                )
            }
            ScoredEvaluationEvent::ScoredEvaluationCompleted {
                node_id, result, ..
            } => {
                format!(
                    "[ScoredEvaluation] Completed: node={}, passed={}, dimensions={}",
                    node_id,
                    result.passed,
                    result.dimensions.len()
                )
            }
            ScoredEvaluationEvent::ScoredEvaluationFailed { node_id, error, .. } => {
                format!(
                    "[ScoredEvaluation] Failed: node={}, error={}",
                    node_id, error
                )
            }
        }
    }

    /// Returns the node_id for this event.
    pub fn node_id(&self) -> &str {
        match self {
            ScoredEvaluationEvent::ScoredEvaluationStarted { node_id, .. } => node_id,
            ScoredEvaluationEvent::ScoredEvaluationCompleted { node_id, .. } => node_id,
            ScoredEvaluationEvent::ScoredEvaluationFailed { node_id, .. } => node_id,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scored_evaluation::domain::result::ScoreDimension;
    use std::collections::HashMap;

    fn make_result() -> ScoringResult {
        let mut dims = HashMap::new();
        dims.insert(
            "correctness".to_string(),
            ScoreDimension::new(0.9, 1.0, "Correctness", true),
        );
        ScoringResult::new(true, dims, "ok", "runtimeai", 100, None)
    }

    #[test]
    fn test_started_event() {
        let event = ScoredEvaluationEvent::ScoredEvaluationStarted {
            node_id: "node-1".to_string(),
            execution_id: Uuid::new_v4(),
            backend: "runtimeai".to_string(),
            timestamp: Utc::now(),
        };
        let log = event.log_line();
        assert!(log.contains("Started"));
        assert!(log.contains("node-1"));
        assert!(log.contains("runtimeai"));
        assert_eq!(event.node_id(), "node-1");
    }

    #[test]
    fn test_completed_event() {
        let event = ScoredEvaluationEvent::ScoredEvaluationCompleted {
            node_id: "node-2".to_string(),
            execution_id: Uuid::new_v4(),
            result: make_result(),
            timestamp: Utc::now(),
        };
        let log = event.log_line();
        assert!(log.contains("Completed"));
        assert!(log.contains("passed=true"));
        assert_eq!(event.node_id(), "node-2");
    }

    #[test]
    fn test_failed_event() {
        let event = ScoredEvaluationEvent::ScoredEvaluationFailed {
            node_id: "node-3".to_string(),
            execution_id: Uuid::new_v4(),
            error: "Backend timeout".to_string(),
            timestamp: Utc::now(),
        };
        let log = event.log_line();
        assert!(log.contains("Failed"));
        assert!(log.contains("Backend timeout"));
        assert_eq!(event.node_id(), "node-3");
    }

    #[test]
    fn test_serialization_roundtrip_started() {
        let event = ScoredEvaluationEvent::ScoredEvaluationStarted {
            node_id: "n1".to_string(),
            execution_id: Uuid::new_v4(),
            backend: "mcp".to_string(),
            timestamp: Utc::now(),
        };
        let json = serde_json::to_string(&event).unwrap();
        let deserialized: ScoredEvaluationEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, deserialized);
        assert!(json.contains("\"type\":\"scored_evaluation_started\""));
    }

    #[test]
    fn test_serialization_roundtrip_completed() {
        let event = ScoredEvaluationEvent::ScoredEvaluationCompleted {
            node_id: "n1".to_string(),
            execution_id: Uuid::new_v4(),
            result: make_result(),
            timestamp: Utc::now(),
        };
        let json = serde_json::to_string(&event).unwrap();
        let deserialized: ScoredEvaluationEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, deserialized);
    }

    #[test]
    fn test_serialization_roundtrip_failed() {
        let event = ScoredEvaluationEvent::ScoredEvaluationFailed {
            node_id: "n1".to_string(),
            execution_id: Uuid::new_v4(),
            error: "error".to_string(),
            timestamp: Utc::now(),
        };
        let json = serde_json::to_string(&event).unwrap();
        let deserialized: ScoredEvaluationEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, deserialized);
    }
}
