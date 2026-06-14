//! TemplateGenerator trait — fallback template generation from user intent.
//!
//! @canonical .pi/architecture/modules/planning-pipeline.md#generator
//! Implements: Contract Freeze — TemplateGenerator trait
//! Issue: issue-contract-freeze
//!
//! TemplateGenerator is the fallback path in the planning pipeline.
//! When the Classifier finds no good match (confidence < 0.3 for all
//! templates), the pipeline falls back to the TemplateGenerator to
//! create a new template definition on-the-fly from the user intent.
//!
//! # Contract (Frozen)
//! - Generates a TOML template string from user intent
//! - The generated template must be parseable by TemplateParserService
//! - The generated template is registered in the TemplateEngine before re-classifying
//! - Implementations must be deterministic (same intent → same template structure)

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::budget_tracking::domain::LlmBudget;
use crate::planning::domain::error::PlanningError;
use crate::planning::domain::intent::UserIntent;

/// Generates a new template definition from user intent.
///
/// Used as a fallback when no existing template matches the user's
/// intent. The generator creates a complete TOML template definition
/// that can be parsed and registered for immediate use.
///
/// # Contract (Frozen)
/// - `generate` returns a TOML string matching the template schema
/// - The output must be parseable by `TemplateParserService::parse_str`
/// - Budget is consumed via `LlmBudget` reservation
/// - Implementations should include appropriate node structure and parameters
#[async_trait]
pub trait TemplateGenerator: Send + Sync {
    /// Generate a template definition from user intent.
    ///
    /// Creates a TOML template string that the TemplateEngine can
    /// parse and register. The generated template should match the
    /// user's intent as closely as possible.
    ///
    /// # Arguments
    ///
    /// * `intent` — The user's raw intent (with optional clarifications).
    /// * `budget` — LLM budget for tracking generation cost.
    ///
    /// # Returns
    ///
    /// A `GeneratedTemplate` containing the TOML string and metadata.
    async fn generate(
        &self,
        intent: &UserIntent,
        budget: &LlmBudget,
    ) -> Result<GeneratedTemplate, PlanningError>;

    /// Estimate the token cost of generating a template.
    ///
    /// Provides a rough estimate for budget pre-checking before
    /// the actual generation call.
    fn estimate_cost(&self, intent: &UserIntent) -> GeneratedTemplateCost;
}

/// A template generated on-the-fly by the TemplateGenerator.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratedTemplate {
    /// The TOML string of the generated template definition.
    pub toml_content: String,

    /// Suggested template ID (used for registration).
    pub suggested_id: String,

    /// Suggested human-readable name.
    pub suggested_name: String,

    /// Brief description of what this template does.
    pub description: String,

    /// Number of LLM calls used.
    pub llm_calls_used: u32,

    /// Number of LLM tokens consumed.
    pub llm_tokens_used: u32,
}

/// Estimated cost of generating a template.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratedTemplateCost {
    /// Estimated number of LLM calls.
    pub estimated_calls: u32,

    /// Estimated number of LLM tokens.
    pub estimated_tokens: u32,
}
