//! ScoredEvaluationNode — DAG node value object for scored evaluation.
//!
//! @canonical .pi/architecture/modules/scored-evaluation.md#scored-evaluation-node
//! Implements: Contract Freeze — ScoredEvaluationNode struct
//! Issue: #673 (scored-evaluation epic)
//!
//! # Contract (Frozen)
//! - Immutable value object after construction
//! - Carries artifact JSON, rubric, backend selector, thresholds, and execution policy
//! - Implements `Clone`, `Debug`, `PartialEq` for testability
//! - Serialization support for DAG template loading

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::rubric::Rubric;

/// DAG node value object for scored evaluation.
///
/// A `ScoredEvaluationNode` is embedded in the DAG as a task node. When
/// executed, the artifact is sent to the configured scoring backend for
/// evaluation against the rubric. Thresholds determine whether each
/// dimension passes or fails.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScoredEvaluationNode {
    /// Unique identifier for this node.
    pub node_id: Uuid,

    /// The artifact to evaluate (typically generated code, patch, or LLM output).
    pub artifact: serde_json::Value,

    /// The rubric to evaluate against (inline or reference).
    pub rubric: Rubric,

    /// Name of the scoring backend to use (e.g., "runtimeai", "custom_http").
    pub backend: String,

    /// Per-dimension score thresholds (dimension name → minimum score 0.0–1.0).
    /// If a dimension's score is below its threshold, the dimension fails.
    #[serde(default)]
    pub thresholds: HashMap<String, f64>,

    /// Execution policy for retry/fallback behavior on evaluation failure.
    #[serde(default)]
    pub policy: ExecutionPolicy,
}

/// Execution policy for scored evaluation nodes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecutionPolicy {
    /// Maximum number of retry attempts on transient failure.
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,

    /// Action to take when evaluation fails after all retries.
    #[serde(default)]
    pub on_failure: FailureAction,
}

/// Action to take on evaluation failure.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureAction {
    /// Retry the evaluation with backoff.
    Retry,
    /// Flag the node for human review (non-blocking).
    FlagForReview,
    /// Block the pipeline execution.
    Block,
}

impl Default for FailureAction {
    fn default() -> Self {
        FailureAction::FlagForReview
    }
}

fn default_max_retries() -> u32 {
    3
}

impl Default for ExecutionPolicy {
    fn default() -> Self {
        Self {
            max_retries: default_max_retries(),
            on_failure: FailureAction::default(),
        }
    }
}

impl ScoredEvaluationNode {
    /// Create a new `ScoredEvaluationNode`.
    pub fn new(
        node_id: Uuid,
        artifact: serde_json::Value,
        rubric: Rubric,
        backend: String,
        thresholds: HashMap<String, f64>,
        policy: ExecutionPolicy,
    ) -> Self {
        Self {
            node_id,
            artifact,
            rubric,
            backend,
            thresholds,
            policy,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scored_evaluation::domain::rubric::RubricSource;

    #[test]
    fn test_new_node() {
        let node_id = Uuid::new_v4();
        let artifact = serde_json::json!({"code": "fn main() {}"});
        let rubric = Rubric::inline(serde_json::json!({"correctness": 0.8}));
        let mut thresholds = HashMap::new();
        thresholds.insert("correctness".to_string(), 0.8);

        let node = ScoredEvaluationNode::new(
            node_id,
            artifact.clone(),
            rubric.clone(),
            "runtimeai".to_string(),
            thresholds.clone(),
            ExecutionPolicy::default(),
        );

        assert_eq!(node.node_id, node_id);
        assert_eq!(node.artifact, artifact);
        assert_eq!(node.rubric, rubric);
        assert_eq!(node.backend, "runtimeai");
        assert_eq!(node.thresholds, thresholds);
        assert_eq!(node.policy, ExecutionPolicy::default());
    }

    #[test]
    fn test_serialization_roundtrip() {
        let node_id = Uuid::new_v4();
        let node = ScoredEvaluationNode::new(
            node_id,
            serde_json::json!({"code": "test"}),
            Rubric::inline(serde_json::json!({"quality": 0.9})),
            "mcp".to_string(),
            HashMap::new(),
            ExecutionPolicy::default(),
        );

        let json = serde_json::to_string(&node).unwrap();
        let deserialized: ScoredEvaluationNode = serde_json::from_str(&json).unwrap();
        assert_eq!(node, deserialized);
    }

    #[test]
    fn test_execution_policy_default() {
        let policy = ExecutionPolicy::default();
        assert_eq!(policy.max_retries, 3);
        assert_eq!(policy.on_failure, FailureAction::FlagForReview);
    }

    #[test]
    fn test_failure_action_default() {
        assert_eq!(FailureAction::default(), FailureAction::FlagForReview);
    }
}
