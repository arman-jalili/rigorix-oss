//! ScoringResult + ScoreDimension — multidimensional scoring result.
//!
//! @canonical .pi/architecture/modules/scored-evaluation.md#scoring-result
//! Implements: Contract Freeze — ScoringResult and ScoreDimension structs
//! Issue: #673 (scored-evaluation epic)
//!
//! # Contract (Frozen)
//! - ScoringResult carries a passed flag, scored dimensions, and backend metadata
//! - ScoreDimension has a float score (0.0–1.0), max, label, and passed flag
//! - Implements `Clone`, `Debug`, `PartialEq` for testability
//! - Serialization support for API responses and persistence

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Multidimensional scoring result returned by a scoring backend.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScoringResult {
    /// Overall pass/fail status (all dimensions passed).
    pub passed: bool,

    /// Map of dimension name to score dimension.
    pub dimensions: HashMap<String, ScoreDimension>,

    /// Human-readable summary of the evaluation.
    pub summary: String,

    /// Name of the backend that produced this result.
    pub backend: String,

    /// Duration of the evaluation in milliseconds.
    pub duration_ms: u64,

    /// Raw response from the backend (for debugging/auditing).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw: Option<serde_json::Value>,
}

/// A single scoring dimension within a scoring result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScoreDimension {
    /// The achieved score (0.0–1.0).
    pub score: f64,

    /// The maximum possible score.
    pub max: f64,

    /// Human-readable label for this dimension.
    pub label: String,

    /// Whether this dimension passed (score >= threshold or score >= max * pass_ratio).
    pub passed: bool,
}

impl ScoringResult {
    /// Create a new scoring result.
    pub fn new(
        passed: bool,
        dimensions: HashMap<String, ScoreDimension>,
        summary: impl Into<String>,
        backend: impl Into<String>,
        duration_ms: u64,
        raw: Option<serde_json::Value>,
    ) -> Self {
        Self {
            passed,
            dimensions,
            summary: summary.into(),
            backend: backend.into(),
            duration_ms,
            raw,
        }
    }

    /// Returns `true` if all dimensions passed.
    pub fn all_dimensions_passed(&self) -> bool {
        self.dimensions.values().all(|d| d.passed)
    }

    /// Returns the number of dimensions.
    pub fn dimension_count(&self) -> usize {
        self.dimensions.len()
    }

    /// Returns the number of passing dimensions.
    pub fn passing_dimension_count(&self) -> usize {
        self.dimensions.values().filter(|d| d.passed).count()
    }

    /// Returns the score for a specific dimension, if present.
    pub fn score_for(&self, dimension: &str) -> Option<f64> {
        self.dimensions.get(dimension).map(|d| d.score)
    }
}

impl ScoreDimension {
    /// Create a new score dimension.
    pub fn new(score: f64, max: f64, label: impl Into<String>, passed: bool) -> Self {
        Self {
            score,
            max,
            label: label.into(),
            passed,
        }
    }

    /// Returns the score as a percentage (0–100).
    pub fn as_percentage(&self) -> u8 {
        if self.max == 0.0 {
            0
        } else {
            ((self.score / self.max) * 100.0) as u8
        }
    }

    /// Evaluate whether this dimension passes against a threshold (0.0–1.0).
    pub fn evaluate(&self, threshold: f64) -> bool {
        self.score >= threshold
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scoring_result_new() {
        let mut dims = HashMap::new();
        dims.insert(
            "correctness".to_string(),
            ScoreDimension::new(0.9, 1.0, "Correctness", true),
        );

        let result = ScoringResult::new(true, dims.clone(), "All good", "runtimeai", 150, None);
        assert!(result.passed);
        assert_eq!(result.dimensions.len(), 1);
        assert_eq!(result.summary, "All good");
        assert_eq!(result.backend, "runtimeai");
        assert_eq!(result.duration_ms, 150);
        assert!(result.raw.is_none());
    }

    #[test]
    fn test_all_dimensions_passed() {
        let mut dims = HashMap::new();
        dims.insert("a".to_string(), ScoreDimension::new(0.9, 1.0, "A", true));
        dims.insert("b".to_string(), ScoreDimension::new(0.8, 1.0, "B", true));

        let result = ScoringResult::new(true, dims, "ok", "test", 0, None);
        assert!(result.all_dimensions_passed());
    }

    #[test]
    fn test_all_dimensions_not_passed() {
        let mut dims = HashMap::new();
        dims.insert("a".to_string(), ScoreDimension::new(0.9, 1.0, "A", true));
        dims.insert("b".to_string(), ScoreDimension::new(0.5, 1.0, "B", false));

        let result = ScoringResult::new(false, dims, "partial", "test", 0, None);
        assert!(!result.all_dimensions_passed());
    }

    #[test]
    fn test_passing_dimension_count() {
        let mut dims = HashMap::new();
        dims.insert("a".to_string(), ScoreDimension::new(0.9, 1.0, "A", true));
        dims.insert("b".to_string(), ScoreDimension::new(0.5, 1.0, "B", false));
        dims.insert("c".to_string(), ScoreDimension::new(0.7, 1.0, "C", true));

        let result = ScoringResult::new(false, dims, "", "test", 0, None);
        assert_eq!(result.passing_dimension_count(), 2);
    }

    #[test]
    fn test_score_for() {
        let mut dims = HashMap::new();
        dims.insert(
            "correctness".to_string(),
            ScoreDimension::new(0.85, 1.0, "Correctness", true),
        );

        let result = ScoringResult::new(true, dims, "", "test", 0, None);
        assert_eq!(result.score_for("correctness"), Some(0.85));
        assert_eq!(result.score_for("nonexistent"), None);
    }

    #[test]
    fn test_score_dimension_as_percentage() {
        let dim = ScoreDimension::new(0.85, 1.0, "Test", true);
        assert_eq!(dim.as_percentage(), 85);
    }

    #[test]
    fn test_score_dimension_as_percentage_zero_max() {
        let dim = ScoreDimension::new(0.0, 0.0, "Test", false);
        assert_eq!(dim.as_percentage(), 0);
    }

    #[test]
    fn test_score_dimension_evaluate() {
        let dim = ScoreDimension::new(0.85, 1.0, "Test", true);
        assert!(dim.evaluate(0.8));
        assert!(!dim.evaluate(0.9));
    }

    #[test]
    fn test_serialization_roundtrip() {
        let mut dims = HashMap::new();
        dims.insert(
            "correctness".to_string(),
            ScoreDimension::new(0.95, 1.0, "Correctness", true),
        );

        let result = ScoringResult::new(
            true,
            dims,
            "All dimensions passed",
            "runtimeai",
            250,
            Some(serde_json::json!({"raw_score": 0.95})),
        );

        let json = serde_json::to_string(&result).unwrap();
        let deserialized: ScoringResult = serde_json::from_str(&json).unwrap();
        assert_eq!(result, deserialized);
    }

    #[test]
    fn test_raw_omitted_when_none() {
        let dims = HashMap::new();
        let result = ScoringResult::new(true, dims, "", "test", 0, None);
        let json = serde_json::to_string(&result).unwrap();
        assert!(!json.contains("raw"));
    }
}
