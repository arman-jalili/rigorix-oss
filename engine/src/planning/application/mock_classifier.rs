//! MockClassifier — test double for offline/CI mode.
//!
//! @canonical .pi/architecture/modules/planning-pipeline.md#mock
//! Implements: PlanningPipeline — MockClassifier test double
//! Issue: issue-planningpipeline
//!
//! Provides a deterministic classifier implementation for testing and CI.
//! Maps known intent phrases to predefined template IDs with configurable
//! confidence scores. Supports all classification scenarios including
//! clarification requests and generator fallback triggers.

use async_trait::async_trait;
use std::collections::HashMap;

use crate::budget_tracking::domain::LlmBudget;
use crate::planning::domain::classification::{
    ClassificationResult, ClassifiedTemplate, Classifier,
};
use crate::planning::domain::error::PlanningError;
use crate::planning::domain::intent::UserIntent;

/// A deterministic classifier for testing and CI environments.
///
/// Maps known intent text patterns to predefined template IDs.
/// Supports configuration of confidence levels to test different
/// pipeline paths (auto-select, clarification, generator fallback).
///
/// # Determinism
///
/// The same `UserIntent` always produces the same classification.
/// This is essential for reproducible tests and CI pipelines
/// that cannot make real LLM calls.
///
/// # Usage
///
/// ```rust,ignore
/// let classifier = MockClassifier::new()
///     .with_match("read file", "template-read-file", 0.95)
///     .with_match("ambiguous", "template-a", 0.45)
///     .with_low_confidence("write", 0.15);
/// ```
pub struct MockClassifier {
    /// Mapping from intent substring → (template_id, confidence, reasoning)
    matches: Vec<(String, String, f64, String)>,
}

impl MockClassifier {
    /// Create a new empty MockClassifier with no matches.
    pub fn new() -> Self {
        Self { matches: Vec::new() }
    }

    /// Register a high-confidence match (auto-select path).
    ///
    /// When an intent contains the `text` substring, classification
    /// returns this template with the given confidence.
    pub fn with_match(
        mut self,
        text: impl Into<String>,
        template_id: impl Into<String>,
        confidence: f64,
    ) -> Self {
        let text_str = text.into();
        let template_id_str = template_id.into();
        let reasoning = format!("Mock: intent contains '{}'", text_str);
        self.matches
            .push((text_str, template_id_str, confidence, reasoning));
        self
    }

    /// Register a match with explicit reasoning string.
    pub fn with_match_and_reasoning(
        mut self,
        text: impl Into<String>,
        template_id: impl Into<String>,
        confidence: f64,
        reasoning: impl Into<String>,
    ) -> Self {
        self.matches
            .push((text.into(), template_id.into(), confidence, reasoning.into()));
        self
    }

    /// Find the best matching template for the given intent.
    fn find_best_match(&self, intent: &UserIntent) -> Vec<ClassifiedTemplate> {
        let mut results: Vec<ClassifiedTemplate> = self
            .matches
            .iter()
            .filter(|(pattern, _, _, _)| intent.input.contains(pattern))
            .map(|(_, template_id, confidence, reasoning)| ClassifiedTemplate {
                template_id: template_id.clone(),
                confidence: *confidence,
                reasoning: reasoning.clone(),
                from_override: false,
            })
            .collect();

        // Sort by confidence descending
        results.sort_by(|a, b| {
            b.confidence
                .partial_cmp(&a.confidence)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        results
    }
}

impl Default for MockClassifier {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Classifier for MockClassifier {
    async fn classify_with_alternatives(
        &self,
        intent: &UserIntent,
        _budget: &LlmBudget,
        _available_templates: &[String],
    ) -> Result<ClassificationResult, PlanningError> {
        let mut alternatives = self.find_best_match(intent);

        if alternatives.is_empty() {
            return Ok(ClassificationResult {
                alternatives: vec![],
                requires_clarification: false,
                needs_generator: true,
                reasoning: "No matching template found in mock classifier".to_string(),
                llm_calls_used: 0,
                llm_tokens_used: 0,
            });
        }

        let top_id = alternatives[0].template_id.clone();
        let top_confidence = alternatives[0].confidence;
        let requires_clarification = (0.3..0.7).contains(&top_confidence);
        let needs_generator = top_confidence < 0.3;

        Ok(ClassificationResult {
            alternatives,
            requires_clarification,
            needs_generator,
            reasoning: format!(
                "Mock classification: top={} confidence={:.2}",
                top_id, top_confidence
            ),
            llm_calls_used: 1,
            llm_tokens_used: 100, // Mock token count
        })
    }
}
