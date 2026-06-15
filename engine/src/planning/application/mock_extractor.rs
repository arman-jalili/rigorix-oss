//! MockParameterExtractor — test double for offline/CI mode.
//!
//! @canonical .pi/architecture/modules/planning-pipeline.md#extractor
//! Implements: PlanningPipeline — MockParameterExtractor test double
//! Issue: issue-planningpipeline
//!
//! Provides a deterministic parameter extractor for testing and CI.
//! Returns predefined parameter values based on intent content.

use async_trait::async_trait;
use std::collections::HashMap;

use crate::budget_tracking::domain::LlmBudget;
use crate::planning::domain::error::PlanningError;
use crate::planning::domain::extractor::{ExtractedParameters, ParameterExtractor};
use crate::planning::domain::intent::UserIntent;

/// A deterministic parameter extractor for testing and CI environments.
///
/// Returns predefined parameter values based on intent content.
/// Supports testing of complete, partial, and missing parameter scenarios.
pub struct MockParameterExtractor {
    /// Default parameters to return for any template.
    default_params: HashMap<String, String>,

    /// Per-template parameter overrides.
    template_overrides: HashMap<String, HashMap<String, String>>,

    /// Parameters to report as missing for a template.
    missing_params: HashMap<String, Vec<String>>,

    /// Whether to simulate an extraction failure.
    simulate_error: bool,
}

impl MockParameterExtractor {
    /// Create a new empty MockParameterExtractor.
    pub fn new() -> Self {
        Self {
            default_params: HashMap::new(),
            template_overrides: HashMap::new(),
            missing_params: HashMap::new(),
            simulate_error: false,
        }
    }

    /// Set default parameters returned for any template.
    pub fn with_default(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.default_params.insert(key.into(), value.into());
        self
    }

    /// Set a template-specific parameter override.
    pub fn with_override(
        mut self,
        template_id: impl Into<String>,
        key: impl Into<String>,
        value: impl Into<String>,
    ) -> Self {
        self.template_overrides
            .entry(template_id.into())
            .or_default()
            .insert(key.into(), value.into());
        self
    }

    /// Mark a parameter as missing for a template.
    pub fn with_missing(
        mut self,
        template_id: impl Into<String>,
        param: impl Into<String>,
    ) -> Self {
        self.missing_params
            .entry(template_id.into())
            .or_default()
            .push(param.into());
        self
    }

    /// Enable simulation of an extraction error.
    pub fn with_error(mut self) -> Self {
        self.simulate_error = true;
        self
    }
}

impl Default for MockParameterExtractor {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ParameterExtractor for MockParameterExtractor {
    async fn extract(
        &self,
        _intent: &UserIntent,
        _budget: &LlmBudget,
        template_id: &str,
        parameter_names: &[String],
    ) -> Result<ExtractedParameters, PlanningError> {
        if self.simulate_error {
            return Err(PlanningError::ExtractionError {
                detail: "Mock extraction error".to_string(),
            });
        }

        let mut parameters = self.default_params.clone();

        // Apply template-specific overrides
        if let Some(overrides) = self.template_overrides.get(template_id) {
            for (k, v) in overrides {
                parameters.insert(k.clone(), v.clone());
            }
        }

        let missing = self
            .missing_params
            .get(template_id)
            .cloned()
            .unwrap_or_default();

        let mut missing_found: Vec<String> = Vec::new();
        for param in parameter_names {
            if !parameters.contains_key(param) {
                if missing.contains(param) {
                    missing_found.push(param.clone());
                } else {
                    // Auto-generate mock values for parameters not explicitly set
                    parameters.insert(param.clone(), format!("mock_{}", param));
                }
            }
        }

        let extra: HashMap<String, String> = parameters
            .keys()
            .filter(|k| !parameter_names.contains(k))
            .map(|k| (k.clone(), parameters.get(k).cloned().unwrap_or_default()))
            .collect();

        let complete = missing_found.is_empty();

        let mut result_params = HashMap::new();
        for param in parameter_names {
            if let Some(val) = parameters.get(param) {
                result_params.insert(param.clone(), val.clone());
            }
        }

        Ok(ExtractedParameters {
            template_id: template_id.to_string(),
            parameters: result_params,
            extra_parameters: extra,
            missing_parameters: missing_found,
            complete,
            reasoning: format!(
                "Mock extraction: {} parameters, complete={}",
                parameter_names.len(),
                complete
            ),
            llm_calls_used: 1,
            llm_tokens_used: 50,
        })
    }
}
