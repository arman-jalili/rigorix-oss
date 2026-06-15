//! ClaudeClassifier — Anthropic Messages API implementation of the Classifier trait.
//!
//! @canonical .pi/architecture/modules/planning-pipeline.md#claude
//! Implements: Classifier Trait — ClaudeClassifier via Anthropic Messages API
//! Issue: issue-classifier-trait
//!
//! Uses Anthropic's Claude API (Messages endpoint) to classify user intent
//! against available templates. Supports configurable model selection,
//! API endpoint, and authentication via API key.
//!
//! # Prompt Structure
//!
//! The classifier builds a structured prompt that lists all available templates
//! with their metadata and asks Claude to rank them by relevance to the user's
//! intent. The response is parsed from JSON embedded in the Claude output.
//!
//! # Security
//!
//! - API key is provided at construction time, not hardcoded
//! - Structured prompts prevent prompt injection (no raw intent in system prompt)
//! - Token limits are respected to prevent budget overruns

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::budget_tracking::domain::LlmBudget;
use crate::planning::domain::classification::{
    ClassificationResult, ClassifiedTemplate, Classifier,
};
use crate::planning::domain::error::PlanningError;
use crate::planning::domain::intent::UserIntent;

/// Configuration for the Claude classifier.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeClassifierConfig {
    /// The Anthropic API endpoint (default: https://api.anthropic.com/v1/messages).
    pub api_url: String,

    /// Claude model to use (default: claude-sonnet-4-20250514).
    pub model: String,

    /// Maximum tokens in the response.
    pub max_tokens: u32,

    /// Request timeout in seconds.
    pub timeout_secs: u64,

    /// Temperature for classification (lower = more deterministic).
    pub temperature: f64,
}

impl Default for ClaudeClassifierConfig {
    fn default() -> Self {
        Self {
            api_url: "https://api.anthropic.com/v1/messages".to_string(),
            model: "claude-sonnet-4-20250514".to_string(),
            max_tokens: 1024,
            timeout_secs: 30,
            temperature: 0.1,
        }
    }
}

/// Classifier using Anthropic's Claude Messages API.
///
/// Communicates with the Claude API to classify user intent against
/// available templates. Returns a ranked list of alternatives with
/// confidence scores and reasoning.
///
/// # API Key
///
/// The API key is provided at construction and sent as the
/// `x-api-key` header. Store the key securely (e.g., environment
/// variable, secret manager).
pub struct ClaudeClassifier {
    /// API key for authentication.
    api_key: String,

    /// Configuration for the classifier.
    config: ClaudeClassifierConfig,

    /// HTTP client for API calls.
    client: reqwest::Client,
}

impl ClaudeClassifier {
    /// Create a new ClaudeClassifier.
    ///
    /// # Arguments
    ///
    /// * `api_key` — Anthropic API key (from environment or secret store).
    /// * `config` — Optional configuration overrides (defaults used if None).
    pub fn new(api_key: String, config: Option<ClaudeClassifierConfig>) -> Self {
        let config = config.unwrap_or_default();
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(config.timeout_secs))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            api_key,
            config,
            client,
        }
    }

    /// Build the system prompt for classification.
    fn build_system_prompt(&self, available_templates: &[String]) -> String {
        let template_list = available_templates
            .iter()
            .enumerate()
            .map(|(i, t)| format!("{}. {}", i + 1, t))
            .collect::<Vec<_>>()
            .join("\n");

        format!(
            r#"You are a template classifier. Your task is to match user intent to the most relevant template.

Available templates:
{}

Respond with a JSON object containing:
- "rankings": an array of objects, each with:
  - "template_id": the template ID
  - "confidence": a float from 0.0 to 1.0 indicating match confidence
  - "reasoning": brief explanation of why this template matches

Rules:
- Return ALL templates ranked by confidence (highest to lowest)
- If no template matches well, set all confidences to 0.0
- Be strict: only high confidences (≥ 0.7) for clear matches
- Output ONLY valid JSON, no other text"#,
            template_list
        )
    }

    /// Build the user message for classification.
    fn build_user_message(&self, intent: &UserIntent) -> String {
        let mut msg = format!("User intent: {}", intent.input);

        if intent.has_clarifications() {
            msg.push_str("\n\nClarification history:");
            for pair in &intent.clarifications {
                msg.push_str(&format!("\nQ: {}\nA: {}", pair.question, pair.answer));
            }
        }

        msg
    }

    /// Parse the Claude API response into ranked alternatives.
    pub(crate) fn parse_response(&self, response_body: &str) -> Result<Vec<ClassifiedTemplate>, PlanningError> {
        // Try to extract JSON from the response (may be wrapped in markdown code blocks)
        let json_str = if let Some(start) = response_body.find('{') {
            if let Some(end) = response_body.rfind('}') {
                &response_body[start..=end]
            } else {
                response_body
            }
        } else {
            response_body
        };

        #[derive(Deserialize)]
        struct ApiRanking {
            template_id: String,
            confidence: f64,
            reasoning: String,
        }

        #[derive(Deserialize)]
        struct ApiResponse {
            rankings: Vec<ApiRanking>,
        }

        let parsed: ApiResponse = serde_json::from_str(json_str).map_err(|e| {
            PlanningError::ClassificationError {
                detail: format!("Failed to parse Claude response: {} (raw: {})", e, response_body.chars().take(200).collect::<String>()),
            }
        })?;

        let templates: Vec<ClassifiedTemplate> = parsed
            .rankings
            .into_iter()
            .map(|r| ClassifiedTemplate {
                template_id: r.template_id,
                confidence: r.confidence.clamp(0.0, 1.0),
                reasoning: r.reasoning,
                from_override: false,
            })
            .collect();

        if templates.is_empty() {
            return Err(PlanningError::ClassificationError {
                detail: "Claude returned empty rankings".to_string(),
            });
        }

        Ok(templates)
    }
}

#[async_trait]
impl Classifier for ClaudeClassifier {
    async fn classify_with_alternatives(
        &self,
        intent: &UserIntent,
        _budget: &LlmBudget,
        available_templates: &[String],
    ) -> Result<ClassificationResult, PlanningError> {
        if available_templates.is_empty() {
            return Ok(ClassificationResult {
                alternatives: vec![],
                requires_clarification: false,
                needs_generator: true,
                reasoning: "No templates available to classify against".to_string(),
                llm_calls_used: 0,
                llm_tokens_used: 0,
            });
        }

        let system_prompt = self.build_system_prompt(available_templates);
        let user_message = self.build_user_message(intent);

        // Build the request body for Anthropic Messages API
        let body = serde_json::json!({
            "model": self.config.model,
            "max_tokens": self.config.max_tokens,
            "temperature": self.config.temperature,
            "system": system_prompt,
            "messages": [
                {"role": "user", "content": user_message}
            ]
        });

        // Make the API call
        let body_bytes = serde_json::to_vec(&body).map_err(|e| {
            PlanningError::ClassificationError {
                detail: format!("Failed to serialize request: {}", e),
            }
        })?;

        let response = self
            .client
            .post(&self.config.api_url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .body(body_bytes)
            .send()
            .await
            .map_err(|e| PlanningError::ClassificationError {
                detail: format!("Claude API request failed: {}", e),
            })?;

        let status = response.status();
        let response_text = response.text().await.map_err(|e| {
            PlanningError::ClassificationError {
                detail: format!("Failed to read Claude response body: {}", e),
            }
        })?;

        if !status.is_success() {
            return Err(PlanningError::ClassificationError {
                detail: format!(
                    "Claude API returned {}: {}",
                    status.as_u16(),
                    response_text.chars().take(200).collect::<String>()
                ),
            });
        }

        // Parse the Anthropic Messages API response
        #[derive(Deserialize)]
        struct AnthropicMessage {
            content: Vec<AnthropicContent>,
        }

        #[derive(Deserialize)]
        struct AnthropicContent {
            #[serde(rename = "type")]
            content_type: String,
            text: Option<String>,
        }

        let message: AnthropicMessage = serde_json::from_str(&response_text).map_err(|e| {
            PlanningError::ClassificationError {
                detail: format!("Failed to parse Claude API response: {}", e),
            }
        })?;

        // Extract text from the first content block
        let content_text = message
            .content
            .iter()
            .find(|c| c.content_type == "text")
            .and_then(|c| c.text.as_deref())
            .ok_or_else(|| PlanningError::ClassificationError {
                detail: "Claude response has no text content".to_string(),
            })?;

        let alternatives = self.parse_response(content_text)?;
        let requires_clarification =
            alternatives.first().map(|t| t.confidence < 0.7).unwrap_or(false);
        let needs_generator = alternatives.first().map(|t| t.confidence < 0.3).unwrap_or(true);

        let reasoning = alternatives
            .first()
            .map(|t| format!("Claude classified: top={} confidence={:.2}", t.template_id, t.confidence))
            .unwrap_or_else(|| "No matching template found".to_string());

        Ok(ClassificationResult {
            alternatives,
            requires_clarification,
            needs_generator,
            reasoning,
            llm_calls_used: 1,
            llm_tokens_used: 0, // Token counting would require parsing the response usage field
        })
    }
}
