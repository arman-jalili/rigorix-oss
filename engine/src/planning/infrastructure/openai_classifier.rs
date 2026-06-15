//! OpenaiClassifier — OpenAI Chat Completions API implementation of the Classifier trait.
//!
//! @canonical .pi/architecture/modules/planning-pipeline.md#openai
//! Implements: Classifier Trait — OpenaiClassifier via OpenAI Chat Completions API
//! Issue: issue-classifier-trait
//!
//! Uses OpenAI's Chat Completions API to classify user intent against available
//! templates. Supports configurable model selection, API endpoint (compatible with
//! any OpenAI-compatible API), and authentication via API key.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::budget_tracking::domain::LlmBudget;
use crate::planning::domain::classification::{
    ClassificationResult, ClassifiedTemplate, Classifier,
};
use crate::planning::domain::error::PlanningError;
use crate::planning::domain::intent::UserIntent;

/// Configuration for the OpenAI classifier.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenaiClassifierConfig {
    /// The OpenAI API endpoint (default: https://api.openai.com/v1/chat/completions).
    pub api_url: String,

    /// Model to use (default: gpt-4o).
    pub model: String,

    /// Maximum tokens in the response.
    pub max_tokens: u32,

    /// Request timeout in seconds.
    pub timeout_secs: u64,

    /// Temperature for classification (lower = more deterministic).
    pub temperature: f64,
}

impl Default for OpenaiClassifierConfig {
    fn default() -> Self {
        Self {
            api_url: "https://api.openai.com/v1/chat/completions".to_string(),
            model: "gpt-4o".to_string(),
            max_tokens: 1024,
            timeout_secs: 30,
            temperature: 0.1,
        }
    }
}

/// Classifier using OpenAI's Chat Completions API.
///
/// Communicates with the OpenAI API (or any OpenAI-compatible endpoint)
/// to classify user intent against available templates. Returns a ranked
/// list of alternatives.
///
/// # API Key
///
/// The API key is provided at construction and sent as the
/// `Authorization: Bearer` header.
pub struct OpenaiClassifier {
    /// API key for authentication.
    api_key: String,

    /// Configuration.
    config: OpenaiClassifierConfig,

    /// HTTP client.
    client: reqwest::Client,
}

impl OpenaiClassifier {
    /// Create a new OpenaiClassifier.
    ///
    /// # Arguments
    ///
    /// * `api_key` — OpenAI API key.
    /// * `config` — Optional configuration overrides.
    pub fn new(api_key: String, config: Option<OpenaiClassifierConfig>) -> Self {
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
            r#"You are a template classifier. Match user intent to the most relevant template.

Available templates:
{}

Respond with a JSON object:
{{
  "rankings": [
    {{
      "template_id": "string",
      "confidence": 0.0-1.0,
      "reasoning": "string"
    }}
  ]
}}

Rules:
- Rank ALL templates by confidence (highest to lowest)
- No match → all confidences 0.0
- Output ONLY valid JSON"#,
            template_list
        )
    }

    /// Build the user message.
    fn build_user_message(&self, intent: &UserIntent) -> String {
        let mut msg = format!("User intent: {}", intent.input);
        if intent.has_clarifications() {
            msg.push_str("\n\nClarifications:");
            for pair in &intent.clarifications {
                msg.push_str(&format!("\nQ: {}\nA: {}", pair.question, pair.answer));
            }
        }
        msg
    }

    /// Parse the API response.
    pub(crate) fn parse_response(&self, response_body: &str) -> Result<Vec<ClassifiedTemplate>, PlanningError> {
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
                detail: format!("Failed to parse OpenAI response: {} (raw: {})", e, 
                    response_body.chars().take(200).collect::<String>()),
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
                detail: "OpenAI returned empty rankings".to_string(),
            });
        }

        Ok(templates)
    }
}

#[async_trait]
impl Classifier for OpenaiClassifier {
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
                reasoning: "No templates available".to_string(),
                llm_calls_used: 0,
                llm_tokens_used: 0,
            });
        }

        let system_prompt = self.build_system_prompt(available_templates);
        let user_message = self.build_user_message(intent);

        let body = serde_json::json!({
            "model": self.config.model,
            "max_tokens": self.config.max_tokens,
            "temperature": self.config.temperature,
            "messages": [
                {"role": "system", "content": system_prompt},
                {"role": "user", "content": user_message}
            ]
        });

        let body_bytes = serde_json::to_vec(&body).map_err(|e| {
            PlanningError::ClassificationError {
                detail: format!("Failed to serialize request: {}", e),
            }
        })?;

        let response = self
            .client
            .post(&self.config.api_url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("content-type", "application/json")
            .body(body_bytes)
            .send()
            .await
            .map_err(|e| PlanningError::ClassificationError {
                detail: format!("OpenAI API request failed: {}", e),
            })?;

        let status = response.status();
        let response_text = response.text().await.map_err(|e| {
            PlanningError::ClassificationError {
                detail: format!("Failed to read OpenAI response body: {}", e),
            }
        })?;

        if !status.is_success() {
            return Err(PlanningError::ClassificationError {
                detail: format!(
                    "OpenAI API returned {}: {}",
                    status.as_u16(),
                    response_text.chars().take(200).collect::<String>()
                ),
            });
        }

        // Parse OpenAI Chat Completions response
        #[derive(Deserialize)]
        struct OpenAiChoice {
            message: OpenAiMessage,
        }

        #[derive(Deserialize)]
        struct OpenAiMessage {
            content: Option<String>,
        }

        #[derive(Deserialize)]
        struct OpenAiResponse {
            choices: Vec<OpenAiChoice>,
            usage: Option<OpenAiUsage>,
        }

        #[derive(Deserialize)]
        #[allow(dead_code)]
        struct OpenAiUsage {
            prompt_tokens: u32,
            completion_tokens: u32,
            total_tokens: u32,
        }

        let api_response: OpenAiResponse = serde_json::from_str(&response_text).map_err(|e| {
            PlanningError::ClassificationError {
                detail: format!("Failed to parse OpenAI response: {}", e),
            }
        })?;

        let content = api_response
            .choices
            .first()
            .and_then(|c| c.message.content.as_deref())
            .ok_or_else(|| PlanningError::ClassificationError {
                detail: "OpenAI response has no content".to_string(),
            })?;

        let alternatives = self.parse_response(content)?;
        let requires_clarification =
            alternatives.first().map(|t| t.confidence < 0.7).unwrap_or(false);
        let needs_generator = alternatives.first().map(|t| t.confidence < 0.3).unwrap_or(true);

        let reasoning = alternatives
            .first()
            .map(|t| format!("OpenAI classified: top={} confidence={:.2}", t.template_id, t.confidence))
            .unwrap_or_else(|| "No matching template found".to_string());

        let tokens_used = api_response
            .usage
            .as_ref()
            .map(|u| u.total_tokens)
            .unwrap_or(0);

        Ok(ClassificationResult {
            alternatives,
            requires_clarification,
            needs_generator,
            reasoning,
            llm_calls_used: 1,
            llm_tokens_used: tokens_used,
        })
    }
}
