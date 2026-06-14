//! ParameterExtractor trait — LLM-based parameter extraction from user intent.
//!
//! @canonical .pi/architecture/modules/planning-pipeline.md#extractor
//! Implements: Contract Freeze — ParameterExtractor trait, ExtractedParameters
//! Issue: issue-contract-freeze
//!
//! The ParameterExtractor is the domain interface for extracting structured
//! parameter values from user intent after a template has been selected by
//! the Classifier.
//!
//! # Contract (Frozen)
//! - The `extract` method is the single entry point
//! - Returns key-value pairs matching the template's ParameterDef list
//! - Missing required parameters are surfaced as `MissingParameter` errors
//! - Implementations must be deterministic (same input → same extraction)
//! - Budget is checked before inference via `LlmBudget` reservation

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::budget_tracking::domain::LlmBudget;
use crate::planning::domain::error::PlanningError;
use crate::planning::domain::intent::UserIntent;

/// Extracts structured parameters from user intent for a matched template.
///
/// After the Classifier selects a template, the ParameterExtractor fills
/// in the template's parameter values by analysing the user's intent text.
///
/// # Contract (Frozen)
/// - Extraction is scoped to a single template (already selected by Classifier)
/// - Returns a flat key-value map matching the template's ParameterDef names
/// - Missing required parameters return `PlanningError::MissingParameter`
/// - Extra parameters found in intent but not in the template are returned
///   as `extra_parameters` for optional handling
#[async_trait]
pub trait ParameterExtractor: Send + Sync {
    /// Extract parameters from user intent for a specific template.
    ///
    /// Analyses the user's intent text and extracts parameter values
    /// matching the given template's parameter definitions.
    ///
    /// # Arguments
    ///
    /// * `intent` — The user's raw intent (with optional clarification history).
    /// * `budget` — The LLM budget for tracking call/token consumption.
    /// * `template_id` — The ID of the template being parameterised.
    /// * `parameter_names` — Ordered list of parameter names to extract.
    ///
    /// # Returns
    ///
    /// An `ExtractedParameters` containing the resolved key-value pairs
    /// and metadata about the extraction process.
    async fn extract(
        &self,
        intent: &UserIntent,
        budget: &LlmBudget,
        template_id: &str,
        parameter_names: &[String],
    ) -> Result<ExtractedParameters, PlanningError>;
}

/// The result of a parameter extraction operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedParameters {
    /// The template ID these parameters were extracted for.
    pub template_id: String,

    /// Resolved key-value pairs of template parameters.
    pub parameters: HashMap<String, String>,

    /// Parameters found in intent but not defined in the template.
    #[serde(default)]
    pub extra_parameters: HashMap<String, String>,

    /// List of required parameters that could not be extracted.
    #[serde(default)]
    pub missing_parameters: Vec<String>,

    /// Whether all required parameters were successfully extracted.
    pub complete: bool,

    /// Human-readable reasoning for the extraction decisions.
    pub reasoning: String,

    /// Number of LLM calls made during this extraction.
    pub llm_calls_used: u32,

    /// Number of LLM tokens consumed during this extraction.
    pub llm_tokens_used: u32,
}
