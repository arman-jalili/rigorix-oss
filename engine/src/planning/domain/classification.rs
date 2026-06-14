//! Classifier trait — LLM-based intent-to-template classification.
//!
//! @canonical .pi/architecture/modules/planning-pipeline.md#classifier
//! Implements: Contract Freeze — Classifier trait, ClassificationResult, ClassifiedTemplate
//! Issue: issue-contract-freeze
//!
//! The Classifier is the core domain interface for mapping user intent to a
//! registered template via LLM inference. It evaluates the intent against all
//! available templates and returns a ranked list of alternatives with confidence
//! scores.
//!
//! # Classification Rules
//!
//! | Confidence Range | Action | Rationale |
//! |-----------------|--------|-----------|
//! | 0.7 – 1.0 | Auto-select | High confidence, proceed to extraction |
//! | 0.3 – 0.7 | Request clarification | Ambiguous, ask user for more context |
//! | 0.0 – 0.3 | Fallback to TemplateGenerator | No match, generate new template |
//!
//! # Contract (Frozen)
//! - The `classify_with_alternatives` method is the single entry point
//! - Returns a ranked list of `ClassifiedTemplate` alternatives
//! - Each alternative carries: template_id, confidence, reasoning
//! - Implementations must be deterministic (same input → same ranking)
//! - Budget is checked before inference via `LlmBudget` reservation
//! - Clarification requests are surfaced through `requires_clarification`

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::budget_tracking::domain::LlmBudget;
use crate::planning::domain::intent::UserIntent;
use crate::planning::domain::error::PlanningError;

/// Classifies user intent against available templates via LLM.
///
/// The classifier evaluates the user's intent against all registered
/// templates and returns a ranked list of alternatives. Each alternative
/// carries the template ID, confidence score, and a reasoning string.
///
/// # Determinism
///
/// Implementations MUST be deterministic for a given model config and
/// template set — the same `UserIntent` and `LlmBudget` MUST always
/// produce the same ranking. This is essential for auditability and
/// replay verification.
///
/// # Budget Integration
///
/// The classifier should call `budget.reserve()` before making an LLM
/// call. The reservation is committed after successful classification
/// or rolled back on failure.
///
/// # TemplateGenerator Fallback
///
/// If the top alternative has confidence < 0.3, the PlanningPipeline
/// should fall back to the TemplateGenerator fallback path. The
/// Classifier returns `needs_generator=true` in this case.
#[async_trait]
pub trait Classifier: Send + Sync {
    /// Classify user intent against available templates.
    ///
    /// Returns a ranked list of `ClassifiedTemplate` alternatives,
    /// ordered from most confident to least. The caller (PlanningPipeline)
    /// uses the top result or requests clarification as needed.
    ///
    /// # Arguments
    ///
    /// * `intent` — The user's raw intent input.
    /// * `budget` — The LLM budget for tracking call/token consumption.
    /// * `available_templates` — List of template IDs to consider.
    ///
    /// # Returns
    ///
    /// A `ClassificationResult` containing the ranked alternatives and
    /// metadata about the classification process.
    async fn classify_with_alternatives(
        &self,
        intent: &UserIntent,
        budget: &LlmBudget,
        available_templates: &[String],
    ) -> Result<ClassificationResult, PlanningError>;

    /// Quick classify — return only the top template, no alternatives.
    ///
    /// Convenience wrapper around `classify_with_alternatives` for
    /// callers who only need the best match.
    async fn classify(
        &self,
        intent: &UserIntent,
        budget: &LlmBudget,
        available_templates: &[String],
    ) -> Result<ClassifiedTemplate, PlanningError> {
        let result = self
            .classify_with_alternatives(intent, budget, available_templates)
            .await?;
        result
            .alternatives
            .into_iter()
            .next()
            .ok_or(PlanningError::ClassificationError {
                detail: "Classifier returned no alternatives".to_string(),
            })
    }
}

/// The complete result of a classification operation.
///
/// Carries the ranked alternatives, whether clarification is needed,
/// and metadata about the LLM call that produced the result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassificationResult {
    /// Ranked list of template alternatives (highest confidence first).
    pub alternatives: Vec<ClassifiedTemplate>,

    /// Whether the classifier requires user clarification.
    ///
    /// Set to `true` when the top alternative's confidence is below
    /// the auto-select threshold but above the generator threshold.
    pub requires_clarification: bool,

    /// Whether the pipeline should fall back to TemplateGenerator.
    ///
    /// Set to `true` when no template matches well enough
    /// (confidence < 0.3 for all alternatives).
    pub needs_generator: bool,

    /// Human-readable reasoning for the classification result.
    ///
    /// Explains why the top template was chosen (or why none matched).
    pub reasoning: String,

    /// Number of LLM calls made during this classification.
    pub llm_calls_used: u32,

    /// Number of LLM tokens consumed during this classification.
    pub llm_tokens_used: u32,
}

/// A single classified template with confidence and reasoning.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassifiedTemplate {
    /// The template ID that was matched.
    pub template_id: String,

    /// Model confidence score (0.0–1.0) for this match.
    pub confidence: f64,

    /// Human-readable explanation of why this template was chosen.
    pub reasoning: String,

    /// Whether this template came from configured overrides
    /// (e.g., explicit template selection) rather than LLM inference.
    pub from_override: bool,
}
