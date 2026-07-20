//! HTTPBackend — HTTP REST adapter for the Rigorix scoring protocol.
//!
//! @canonical .pi/architecture/modules/scored-evaluation.md#http-backend
//! Implements: Contract Freeze — HttpBackend stub
//! Issue: #673 (scored-evaluation epic)
//!
//! # Contract (Frozen)
//! - Implements the `ScoringBackend` trait
//! - POSTs artifact + rubric to configurable URL
//! - Expects `ScoringResult`-compatible JSON response
//! - Configurable timeout, headers, and auth
//! - Implements health check via HEAD or GET to health endpoint
//!
//! # Implementation Notes (TODO)
//! - Use reqwest for HTTP client
//! - POST with JSON body containing artifact + rubric
//! - Parse response into ScoringResult
//! - Support auth headers (Bearer token, custom header)
//! - Configurable timeout per request

use async_trait::async_trait;
use std::collections::HashMap;

use crate::scored_evaluation::domain::{
    ScoredEvaluationError, ScoringBackend, ScoringResult, Rubric,
};

/// HTTP REST adapter for the Rigorix scoring protocol.
///
/// POSTs scoring requests to a configurable HTTP endpoint and parses
/// the JSON response into a `ScoringResult`.
pub struct HttpBackend {
    /// Name identifier for this backend instance.
    name: &'static str,
    /// Scoring endpoint URL.
    url: String,
    /// Custom HTTP headers to include in requests.
    headers: HashMap<String, String>,
    /// Request timeout in milliseconds.
    timeout_ms: u64,
    /// Optional URL for health checks (defaults to same URL).
    health_url: Option<String>,
}

impl HttpBackend {
    /// Create a new HTTP backend adapter.
    pub fn new(
        url: impl Into<String>,
        headers: HashMap<String, String>,
        timeout_ms: u64,
    ) -> Self {
        Self {
            name: "http",
            url: url.into(),
            headers,
            timeout_ms,
            health_url: None,
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
        _artifact: &serde_json::Value,
        _rubric: &Rubric,
    ) -> Result<ScoringResult, ScoredEvaluationError> {
        // TODO: implement HTTP call
        // 1. POST artifact + rubric as JSON to self.url
        // 2. Parse response into ScoringResult
        // 3. Handle timeout, HTTP errors, invalid responses
        Err(ScoredEvaluationError::Internal(
            "HTTPBackend not yet implemented".to_string(),
        ))
    }

    fn backend_name(&self) -> &'static str {
        self.name
    }

    async fn health_check(&self) -> Result<bool, ScoredEvaluationError> {
        // TODO: implement health check
        // HEAD or GET to health_url (or self.url), check status 200
        Err(ScoredEvaluationError::Internal(
            "HTTPBackend health check not yet implemented".to_string(),
        ))
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
        let backend = HttpBackend::new("https://evaluate.example.com/score", headers.clone(), 10_000);
        assert_eq!(backend.headers().get("Authorization").unwrap(), "Bearer token123");
    }
}
