//! LLM Provider client implementations.
//!
//! @canonical .pi/architecture/modules/llm-step.md
//! Implements: LlmGenerateNode — LlmProviderClient HTTP implementations
//! Issue: issue-llmgeneratenode
//!
//! Concrete implementations of LlmProviderClient for various LLM providers.
//! Currently supports a mock client for testing and an HTTP-based client
//! for real provider integration.

use async_trait::async_trait;
use serde_json::json;

use crate::llm_step::application::factory::{
    LlmProviderClient, LlmProviderRequest, LlmProviderResponse,
};
use crate::llm_step::domain::LlmStepError;

/// A mock LLM provider client for testing.
///
/// Returns configurable responses without making HTTP calls.
/// Useful for unit tests and development.
#[derive(Debug)]
pub struct MockLlmProviderClient {
    /// The response to return for all generation requests.
    response: LlmProviderResponse,
    /// Whether to simulate a failure.
    fail: bool,
    /// The error to return if fail is true.
    error: Option<LlmStepError>,
}

impl MockLlmProviderClient {
    /// Create a new MockLlmProviderClient with a fixed response.
    pub fn new(response: LlmProviderResponse) -> Self {
        Self {
            response,
            fail: false,
            error: None,
        }
    }

    /// Make this client simulate a failure on generate.
    pub fn with_failure(mut self, error: LlmStepError) -> Self {
        self.fail = true;
        self.error = Some(error);
        self
    }
}

#[async_trait]
impl LlmProviderClient for MockLlmProviderClient {
    async fn generate(
        &self,
        _request: LlmProviderRequest,
    ) -> Result<LlmProviderResponse, LlmStepError> {
        if self.fail {
            return Err(self.error.clone().unwrap_or(LlmStepError::ProviderError {
                provider: "mock".to_string(),
                status: 500,
                message: "Mock provider failure".to_string(),
            }));
        }
        Ok(self.response.clone())
    }

    async fn health_check(&self) -> Result<bool, LlmStepError> {
        Ok(!self.fail)
    }
}

/// HTTP-based LLM provider client for Anthropic (Claude) API.
///
/// Makes HTTP requests to the Anthropic Messages API endpoint.
/// Supports configurable timeouts, retries, and token tracking.
#[derive(Debug)]
pub struct AnthropicProviderClient {
    /// The API endpoint URL.
    api_url: String,
    /// The API key for authentication.
    api_key: String,
    /// Request timeout in seconds.
    #[allow(dead_code)]
    timeout_secs: u64,
    /// HTTP client.
    client: reqwest::Client,
}

impl AnthropicProviderClient {
    /// Create a new AnthropicProviderClient.
    pub fn new(api_url: String, api_key: String, timeout_secs: u64) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(timeout_secs))
            .build()
            .unwrap_or_default();

        Self {
            api_url,
            api_key,
            timeout_secs,
            client,
        }
    }

    /// Build the Anthropic API request body.
    fn build_request(&self, request: &LlmProviderRequest) -> serde_json::Value {
        json!({
            "model": request.model,
            "max_tokens": request.max_tokens,
            "temperature": request.temperature,
            "system": request.system_prompt,
            "messages": [
                {
                    "role": "user",
                    "content": request.user_message
                }
            ]
        })
    }
}

#[async_trait]
impl LlmProviderClient for AnthropicProviderClient {
    async fn generate(
        &self,
        request: LlmProviderRequest,
    ) -> Result<LlmProviderResponse, LlmStepError> {
        let body = self.build_request(&request);

        let response = self
            .client
            .post(&self.api_url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| LlmStepError::ProviderError {
                provider: "anthropic".to_string(),
                status: 0,
                message: format!("HTTP request failed: {}", e),
            })?;

        let status = response.status().as_u16();
        if status >= 400 {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(LlmStepError::ProviderError {
                provider: "anthropic".to_string(),
                status,
                message: error_text,
            });
        }

        let response_json: serde_json::Value =
            response
                .json()
                .await
                .map_err(|e| LlmStepError::ProviderError {
                    provider: "anthropic".to_string(),
                    status: 0,
                    message: format!("Failed to parse response: {}", e),
                })?;

        // Extract content from Anthropic response format
        let content = response_json["content"]
            .as_array()
            .and_then(|arr| arr.first())
            .and_then(|block| block["text"].as_str())
            .unwrap_or("")
            .to_string();

        let model = response_json["model"]
            .as_str()
            .unwrap_or(&request.model)
            .to_string();

        let usage = &response_json["usage"];
        let prompt_tokens = usage["input_tokens"].as_u64().unwrap_or(0) as u32;
        let completion_tokens = usage["output_tokens"].as_u64().unwrap_or(0) as u32;

        let stop_reason = response_json["stop_reason"]
            .as_str()
            .unwrap_or("end_turn")
            .to_string();

        Ok(LlmProviderResponse {
            content,
            model,
            prompt_tokens,
            completion_tokens,
            stop_reason,
            provider_metadata: Some(response_json),
        })
    }

    async fn health_check(&self) -> Result<bool, LlmStepError> {
        // Simple health check: try to list models or validate auth
        let response = self
            .client
            .get(&self.api_url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .send()
            .await
            .map_err(|_| LlmStepError::ProviderError {
                provider: "anthropic".to_string(),
                status: 0,
                message: "Health check failed".to_string(),
            })?;

        Ok(response.status().is_success())
    }
}

/// HTTP-based LLM provider client for OpenAI API.
///
/// Makes HTTP requests to the OpenAI Chat Completions API endpoint.
/// Supports configurable timeouts and token tracking.
#[derive(Debug)]
pub struct OpenAiProviderClient {
    /// The API endpoint URL.
    api_url: String,
    /// The API key for authentication.
    api_key: String,
    /// Request timeout in seconds.
    #[allow(dead_code)]
    timeout_secs: u64,
    /// HTTP client.
    client: reqwest::Client,
}

impl OpenAiProviderClient {
    /// Create a new OpenAiProviderClient.
    pub fn new(api_url: String, api_key: String, timeout_secs: u64) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(timeout_secs))
            .build()
            .unwrap_or_default();

        Self {
            api_url,
            api_key,
            timeout_secs,
            client,
        }
    }

    /// Build the OpenAI API request body.
    fn build_request(&self, request: &LlmProviderRequest) -> serde_json::Value {
        let messages = if request.system_prompt.is_empty() {
            json!([
                {"role": "user", "content": request.user_message}
            ])
        } else {
            json!([
                {"role": "system", "content": request.system_prompt},
                {"role": "user", "content": request.user_message}
            ])
        };

        json!({
            "model": request.model,
            "max_tokens": request.max_tokens,
            "temperature": request.temperature,
            "top_p": request.top_p,
            "messages": messages
        })
    }
}

#[async_trait]
impl LlmProviderClient for OpenAiProviderClient {
    async fn generate(
        &self,
        request: LlmProviderRequest,
    ) -> Result<LlmProviderResponse, LlmStepError> {
        let body = self.build_request(&request);

        let response = self
            .client
            .post(&self.api_url)
            .header("Authorization", format!("Bearer {}", &self.api_key))
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| LlmStepError::ProviderError {
                provider: "openai".to_string(),
                status: 0,
                message: format!("HTTP request failed: {}", e),
            })?;

        let status = response.status().as_u16();
        if status >= 400 {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(LlmStepError::ProviderError {
                provider: "openai".to_string(),
                status,
                message: error_text,
            });
        }

        let response_json: serde_json::Value =
            response
                .json()
                .await
                .map_err(|e| LlmStepError::ProviderError {
                    provider: "openai".to_string(),
                    status: 0,
                    message: format!("Failed to parse response: {}", e),
                })?;

        // Extract content from OpenAI response format
        let choice = &response_json["choices"][0];
        let content = choice["message"]["content"]
            .as_str()
            .unwrap_or("")
            .to_string();

        let model = response_json["model"]
            .as_str()
            .unwrap_or(&request.model)
            .to_string();

        let usage = &response_json["usage"];
        let prompt_tokens = usage["prompt_tokens"].as_u64().unwrap_or(0) as u32;
        let completion_tokens = usage["completion_tokens"].as_u64().unwrap_or(0) as u32;

        let finish_reason = choice["finish_reason"]
            .as_str()
            .unwrap_or("stop")
            .to_string();

        Ok(LlmProviderResponse {
            content,
            model,
            prompt_tokens,
            completion_tokens,
            stop_reason: finish_reason,
            provider_metadata: Some(response_json),
        })
    }

    async fn health_check(&self) -> Result<bool, LlmStepError> {
        let response = self
            .client
            .get(&self.api_url)
            .header("Authorization", format!("Bearer {}", &self.api_key))
            .send()
            .await
            .map_err(|_| LlmStepError::ProviderError {
                provider: "openai".to_string(),
                status: 0,
                message: "Health check failed".to_string(),
            })?;

        Ok(response.status().is_success())
    }
}

impl Default for MockLlmProviderClient {
    fn default() -> Self {
        Self::new(LlmProviderResponse {
            content: String::from("Mock generated content"),
            model: String::from("mock-model"),
            prompt_tokens: 10,
            completion_tokens: 20,
            stop_reason: String::from("end_turn"),
            provider_metadata: None,
        })
    }
}
