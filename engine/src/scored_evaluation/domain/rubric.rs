//! Rubric — evaluation rubric for scored evaluation nodes.
//!
//! @canonical .pi/architecture/modules/scored-evaluation.md#rubric
//! Implements: Contract Freeze — Rubric struct and RubricSource enum
//! Issue: #673 (scored-evaluation epic)
//!
//! # Contract (Frozen)
//! - Rubric can be inline JSON or a reference to an external file/URL
//! - RubricSource uses serde tagged enum for serialization clarity
//! - `scenario_id` is optional for systems that pre-define scenarios
//! - Implements `Clone`, `Debug`, `PartialEq` for testability

use serde::{Deserialize, Serialize};

/// Evaluation rubric — the criteria against which an artifact is scored.
///
/// A rubric defines what dimensions to evaluate (correctness, completeness,
/// style, etc.) and optionally references a pre-defined scenario on the
/// scoring backend.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Rubric {
    /// The source of the rubric content.
    pub source: RubricSource,

    /// Optional scenario identifier for backends that pre-define scenarios
    /// (e.g., RuntimeAI scenario IDs).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scenario_id: Option<String>,
}

/// Source of rubric content — either inline JSON or a reference.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RubricSource {
    /// The rubric content is provided inline as JSON.
    Inline {
        /// The JSON rubric content (dimensions, descriptions, weights).
        content: serde_json::Value,
    },
    /// The rubric is referenced by path or URL.
    Reference {
        /// File path or URL to the rubric definition.
        path_or_url: String,
    },
}

impl Rubric {
    /// Create a new inline rubric.
    pub fn inline(content: serde_json::Value) -> Self {
        Self {
            source: RubricSource::Inline { content },
            scenario_id: None,
        }
    }

    /// Create a new reference rubric.
    pub fn reference(path_or_url: impl Into<String>) -> Self {
        Self {
            source: RubricSource::Reference {
                path_or_url: path_or_url.into(),
            },
            scenario_id: None,
        }
    }

    /// Set an optional scenario identifier.
    pub fn with_scenario_id(mut self, scenario_id: impl Into<String>) -> Self {
        self.scenario_id = Some(scenario_id.into());
        self
    }

    /// Returns `true` if the rubric source is inline.
    pub fn is_inline(&self) -> bool {
        matches!(self.source, RubricSource::Inline { .. })
    }

    /// Returns `true` if the rubric source is a reference.
    pub fn is_reference(&self) -> bool {
        matches!(self.source, RubricSource::Reference { .. })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inline_rubric() {
        let content = serde_json::json!({
            "correctness": {"description": "Is the code correct?", "weight": 0.5},
            "completeness": {"description": "Are all requirements met?", "weight": 0.5}
        });
        let rubric = Rubric::inline(content.clone());
        assert!(rubric.is_inline());
        assert!(!rubric.is_reference());
        assert_eq!(rubric.source, RubricSource::Inline { content });
        assert!(rubric.scenario_id.is_none());
    }

    #[test]
    fn test_reference_rubric() {
        let rubric = Rubric::reference("./rubrics/code_quality.json");
        assert!(!rubric.is_inline());
        assert!(rubric.is_reference());
        assert_eq!(
            rubric.source,
            RubricSource::Reference {
                path_or_url: "./rubrics/code_quality.json".to_string()
            }
        );
    }

    #[test]
    fn test_with_scenario_id() {
        let rubric = Rubric::inline(serde_json::json!({}))
            .with_scenario_id("scenario-abc-123");
        assert_eq!(
            rubric.scenario_id,
            Some("scenario-abc-123".to_string())
        );
    }

    #[test]
    fn test_serialization_roundtrip_inline() {
        let rubric = Rubric::inline(serde_json::json!({"dim": 0.8}))
            .with_scenario_id("s-1");
        let json = serde_json::to_string(&rubric).unwrap();
        let deserialized: Rubric = serde_json::from_str(&json).unwrap();
        assert_eq!(rubric, deserialized);
        // Verify tagged enum serialization
        assert!(json.contains("\"type\":\"inline\""));
    }

    #[test]
    fn test_serialization_roundtrip_reference() {
        let rubric = Rubric::reference("https://example.com/rubric.json");
        let json = serde_json::to_string(&rubric).unwrap();
        let deserialized: Rubric = serde_json::from_str(&json).unwrap();
        assert_eq!(rubric, deserialized);
        assert!(json.contains("\"type\":\"reference\""));
    }

    #[test]
    fn test_scenario_id_omitted_when_none() {
        let rubric = Rubric::inline(serde_json::json!({}));
        let json = serde_json::to_string(&rubric).unwrap();
        assert!(!json.contains("scenario_id"));
    }
}
