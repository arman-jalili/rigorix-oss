//! Data Transfer Objects for the Scored Evaluation module.
//!
//! @canonical .pi/architecture/modules/scored-evaluation.md
//! Implements: Contract Freeze — DTO schemas for scored evaluation
//! Issue: #673 (scored-evaluation epic)
//!
//! DTOs define the input/output contracts for service operations.
//! They carry validation metadata and documentation but no behavior.
//!
//! # Contract (Frozen)
//! - Every service operation has a dedicated input and output DTO
//! - DTOs are serializable (JSON for API and persistence)
//! - Validation constraints are documented in field docs

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::scored_evaluation::domain::{Rubric, ScoringResult};

/// Input for evaluating an artifact against a rubric.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluateInput {
    /// The artifact to evaluate (typically a JSON value).
    pub artifact: serde_json::Value,

    /// The rubric to evaluate against.
    pub rubric: Rubric,

    /// Execution context for traceability.
    pub context: EvaluationContext,

    /// The scoring backend to use (GAP-A-13).
    ///
    /// `None` selects the first configured backend (default); `Some(name)`
    /// selects the named backend or fails with `BackendNotFound`.
    #[serde(default)]
    pub backend: Option<String>,
}

/// Execution context for an evaluation request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluationContext {
    /// The execution this evaluation belongs to.
    pub execution_id: Uuid,

    /// The DAG node being evaluated.
    pub node_id: Uuid,

    /// Human-readable name of the node.
    pub node_name: String,
}

/// Output from evaluating an artifact.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvaluateOutput {
    /// The scoring result from the backend.
    pub result: ScoringResult,

    /// The execution this evaluation belongs to.
    pub execution_id: Uuid,

    /// The DAG node that was evaluated.
    pub node_id: Uuid,

    /// Human-readable name of the node.
    pub node_name: String,

    /// When the evaluation completed.
    pub timestamp: DateTime<Utc>,
}

impl EvaluateInput {
    /// Create a new evaluate input.
    pub fn new(
        artifact: serde_json::Value,
        rubric: Rubric,
        execution_id: Uuid,
        node_id: Uuid,
        node_name: impl Into<String>,
    ) -> Self {
        Self {
            artifact,
            rubric,
            context: EvaluationContext {
                execution_id,
                node_id,
                node_name: node_name.into(),
            },
            backend: None,
        }
    }

    /// Select a specific scoring backend for this evaluation.
    pub fn with_backend(mut self, backend: impl Into<String>) -> Self {
        self.backend = Some(backend.into());
        self
    }
}

impl EvaluateOutput {
    /// Create a new evaluate output.
    pub fn new(
        result: ScoringResult,
        execution_id: Uuid,
        node_id: Uuid,
        node_name: impl Into<String>,
        timestamp: DateTime<Utc>,
    ) -> Self {
        Self {
            result,
            execution_id,
            node_id,
            node_name: node_name.into(),
            timestamp,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scored_evaluation::domain::Rubric;

    #[test]
    fn test_evaluate_input_new() {
        let input = EvaluateInput::new(
            serde_json::json!({"code": "test"}),
            Rubric::inline(serde_json::json!({"quality": 0.9})),
            Uuid::new_v4(),
            Uuid::new_v4(),
            "score_output",
        );
        assert_eq!(input.context.node_name, "score_output");
    }

    #[test]
    fn test_serialization_roundtrip_input() {
        let input = EvaluateInput::new(
            serde_json::json!({"code": "test"}),
            Rubric::reference("./rubric.json"),
            Uuid::new_v4(),
            Uuid::new_v4(),
            "my-node",
        );
        let json = serde_json::to_string(&input).unwrap();
        let deserialized: EvaluateInput = serde_json::from_str(&json).unwrap();
        assert_eq!(input.context.node_name, deserialized.context.node_name);
        assert_eq!(input.artifact, deserialized.artifact);
    }

    #[test]
    fn test_evaluate_output_new() {
        use std::collections::HashMap;
        let dims = HashMap::new();
        let result = ScoringResult::new(true, dims, "ok", "test", 0, None);
        let output = EvaluateOutput::new(
            result,
            Uuid::new_v4(),
            Uuid::new_v4(),
            "score_output",
            Utc::now(),
        );
        assert_eq!(output.node_name, "score_output");
    }
}
