//! HTTPBackend — HTTP REST adapter for the Rigorix scoring protocol.
//!
//! @canonical .pi/architecture/modules/scored-evaluation.md#http-backend
//! Implements: HTTP REST adapter for Rigorix scoring protocol
//! Issue: #689 (scored-evaluation epic)
//!
//! POSTs artifact + rubric to a configurable HTTP endpoint per the Rigorix
//! scoring protocol and parses the JSON response into ScoringResult.

use async_trait::async_trait;
use std::collections::HashMap;

use crate::scored_evaluation::domain::{
    Rubric, ScoredEvaluationError, ScoringBackend, ScoringResult,
};

/// HTTP REST adapter for the Rigorix scoring protocol.
///
/// POSTs scoring requests to a configurable HTTP endpoint and parses
/// the JSON response into a `ScoringResult`. Supports custom headers,
/// bearer auth, and configurable timeout.
pub struct HttpBackend {
    name: &'static str,
    url: String,
    headers: HashMap<String, String>,
    timeout_ms: u64,
    health_url: Option<String>,
    client: reqwest::Client,
}

impl HttpBackend {
    /// Create a new HTTP backend adapter.
    pub fn new(url: impl Into<String>, headers: HashMap<String, String>, timeout_ms: u64) -> Self {
        let timeout = std::time::Duration::from_millis(timeout_ms);
        let mut client_builder = reqwest::Client::builder().timeout(timeout);

        if let Some(auth) = headers.get("Authorization")
            && let Some(_bearer) = auth.strip_prefix("Bearer ") {
                let mut auth_value = reqwest::header::HeaderValue::try_from(auth.as_str()).ok();
                if let Some(val) = auth_value.as_mut() {
                    client_builder = client_builder.default_headers({
                        let mut h = reqwest::header::HeaderMap::new();
                        h.insert(reqwest::header::AUTHORIZATION, val.clone());
                        h
                    });
                }
            }

        Self {
            name: "http",
            url: url.into(),
            headers,
            timeout_ms,
            health_url: None,
            client: client_builder.build().unwrap_or_default(),
        }
    }

    /// Set a custom health check URL.
    pub fn with_health_url(mut self, url: impl Into<String>) -> Self {
        self.health_url = Some(url.into());
        self
    }

    /// Returns the scoring endpoint URL.
    pub fn url(&self) -> &str {
        &self.url
    }

    /// Returns the custom headers.
    pub fn headers(&self) -> &HashMap<String, String> {
        &self.headers
    }

    /// Returns the timeout in milliseconds.
    pub fn timeout_ms(&self) -> u64 {
        self.timeout_ms
    }
}

#[async_trait]
impl ScoringBackend for HttpBackend {
    async fn evaluate(
        &self,
        artifact: &serde_json::Value,
        rubric: &Rubric,
    ) -> Result<ScoringResult, ScoredEvaluationError> {
        let payload = serde_json::json!({
            "artifact": artifact,
            "rubric": rubric,
        });

        let mut request = self.client.post(&self.url).json(&payload);

        // Add custom headers
        for (key, value) in &self.headers {
            if key != "Authorization"
                && let Ok(header_name) = key.parse::<reqwest::header::HeaderName>()
                    && let Ok(header_value) = value.parse::<reqwest::header::HeaderValue>() {
                        request = request.header(header_name, header_value);
                    }
        }

        let response = request.send().await.map_err(|e| {
            if e.is_timeout() {
                ScoredEvaluationError::Timeout(self.timeout_ms)
            } else {
                ScoredEvaluationError::BackendError(format!("HTTP request failed: {}", e))
            }
        })?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(ScoredEvaluationError::BackendError(format!(
                "HTTP {}: {}",
                status, body
            )));
        }

        let scoring_result: ScoringResult = response.json().await.map_err(|e| {
            ScoredEvaluationError::BackendError(format!("Invalid ScoringResult response: {}", e))
        })?;

        Ok(scoring_result)
    }

    fn backend_name(&self) -> &'static str {
        self.name
    }

    async fn health_check(&self) -> Result<bool, ScoredEvaluationError> {
        let health_url = self.health_url.as_deref().unwrap_or(&self.url);
        match self.client.head(health_url).send().await {
            Ok(response) => Ok(response.status().is_success()),
            Err(_) => Ok(false),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_backend() {
        let headers = HashMap::new();
        let backend = HttpBackend::new("https://evaluate.example.com/score", headers, 30_000);
        assert_eq!(backend.backend_name(), "http");
        assert_eq!(backend.url(), "https://evaluate.example.com/score");
        assert_eq!(backend.timeout_ms(), 30_000);
        assert!(backend.health_url.is_none());
    }

    #[test]
    fn test_with_health_url() {
        let headers = HashMap::new();
        let backend = HttpBackend::new("https://evaluate.example.com/score", headers, 30_000)
            .with_health_url("https://evaluate.example.com/health");
        assert_eq!(
            backend.health_url.unwrap(),
            "https://evaluate.example.com/health"
        );
    }

    #[test]
    fn test_custom_headers() {
        let mut headers = HashMap::new();
        headers.insert("Authorization".to_string(), "Bearer token123".to_string());
        headers.insert("X-Custom".to_string(), "value".to_string());
        let backend = HttpBackend::new(
            "https://evaluate.example.com/score",
            headers.clone(),
            10_000,
        );
        assert_eq!(
            backend.headers().get("Authorization").unwrap(),
            "Bearer token123"
        );
        assert_eq!(backend.headers().get("X-Custom").unwrap(), "value");
    }

    #[test]
    fn test_serialization_payload() {
        let payload = serde_json::json!({
            "artifact": {"code": "fn main() {}"},
            "rubric": {"source": {"type": "inline", "content": {"quality": 0.9}}, "scenario_id": null},
        });
        let json = serde_json::to_string(&payload).unwrap();
        assert!(json.contains("artifact"));
        assert!(json.contains("rubric"));
    }
}
