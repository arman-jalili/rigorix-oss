//! MCPBackend — MCP protocol adapter for the Rigorix scoring protocol.
//!
//! @canonical .pi/architecture/modules/scored-evaluation.md#mcp-backend
//! Implements: MCP protocol adapter for Rigorix scoring protocol
//! Issue: #688 (scored-evaluation epic)
//!
//! Sends `rigorix_evaluate_artifact` requests over MCP (JSON-RPC) to any
//! server implementing the Rigorix scoring protocol. External systems like
//! RuntimeAI adopt this protocol by implementing the server side.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::scored_evaluation::domain::{
    ScoredEvaluationError, ScoringBackend, ScoringResult, Rubric,
};

/// JSON-RPC request for artifact evaluation.
#[derive(Debug, Serialize)]
struct EvaluateRequest {
    jsonrpc: String,
    method: String,
    params: serde_json::Value,
    id: u64,
}

/// JSON-RPC response for artifact evaluation.
#[derive(Debug, Deserialize)]
struct EvaluateResponse {
    jsonrpc: String,
    result: Option<serde_json::Value>,
    error: Option<JsonRpcError>,
    id: u64,
}

/// JSON-RPC error object.
#[derive(Debug, Deserialize)]
struct JsonRpcError {
    code: i64,
    message: String,
}

/// Ping request for health check.
#[derive(Debug, Serialize)]
struct PingRequest {
    jsonrpc: String,
    method: String,
    params: serde_json::Value,
    id: u64,
}

/// Ping response.
#[derive(Debug, Deserialize)]
struct PingResponse {
    jsonrpc: String,
    result: Option<serde_json::Value>,
    error: Option<JsonRpcError>,
    id: u64,
}

/// MCP protocol adapter for the Rigorix scoring protocol.
///
/// Connects to any MCP server implementing the `rigorix_evaluate_artifact`
/// and `rigorix_ping` methods of the Rigorix scoring protocol.
pub struct McpBackend {
    name: &'static str,
    endpoint: String,
    timeout_ms: u64,
    client: reqwest::Client,
}

impl McpBackend {
    /// Create a new MCP backend adapter.
    pub fn new(endpoint: impl Into<String>, timeout_ms: u64) -> Self {
        let timeout = std::time::Duration::from_millis(timeout_ms);
        let client = reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .unwrap_or_default();

        Self {
            name: "mcp",
            endpoint: endpoint.into(),
            timeout_ms,
            client,
        }
    }

    /// Returns the endpoint URL.
    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    /// Returns the timeout in milliseconds.
    pub fn timeout_ms(&self) -> u64 {
        self.timeout_ms
    }
}

#[async_trait]
impl ScoringBackend for McpBackend {
    async fn evaluate(
        &self,
        artifact: &serde_json::Value,
        rubric: &Rubric,
    ) -> Result<ScoringResult, ScoredEvaluationError> {
        let request = EvaluateRequest {
            jsonrpc: "2.0".to_string(),
            method: "rigorix_evaluate_artifact".to_string(),
            params: serde_json::json!({
                "artifact": artifact,
                "rubric": rubric,
            }),
            id: 1,
        };

        let response = self
            .client
            .post(&self.endpoint)
            .json(&request)
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    ScoredEvaluationError::Timeout(self.timeout_ms)
                } else {
                    ScoredEvaluationError::BackendError(e.to_string())
                }
            })?;

        if !response.status().is_success() {
            return Err(ScoredEvaluationError::BackendError(format!(
                "MCP server returned status {}",
                response.status()
            )));
        }

        let rpc_response: EvaluateResponse = response.json().await.map_err(|e| {
            ScoredEvaluationError::BackendError(format!("Invalid MCP response: {}", e))
        })?;

        if let Some(err) = rpc_response.error {
            return Err(ScoredEvaluationError::BackendError(format!(
                "MCP error ({}): {}",
                err.code, err.message
            )));
        }

        let result_value = rpc_response
            .result
            .ok_or_else(|| ScoredEvaluationError::BackendError("Empty MCP response".to_string()))?;

        let scoring_result: ScoringResult = serde_json::from_value(result_value).map_err(|e| {
            ScoredEvaluationError::BackendError(format!(
                "Invalid ScoringResult from MCP: {}",
                e
            ))
        })?;

        Ok(scoring_result)
    }

    fn backend_name(&self) -> &'static str {
        self.name
    }

    async fn health_check(&self) -> Result<bool, ScoredEvaluationError> {
        let request = PingRequest {
            jsonrpc: "2.0".to_string(),
            method: "rigorix_ping".to_string(),
            params: serde_json::json!({}),
            id: 1,
        };

        match self.client.post(&self.endpoint).json(&request).send().await {
            Ok(response) => {
                if !response.status().is_success() {
                    return Ok(false);
                }
                let ping: PingResponse = response.json().await.map_err(|_| {
                    ScoredEvaluationError::BackendError("Invalid ping response".to_string())
                })?;
                Ok(ping.error.is_none() && ping.result.is_some())
            }
            Err(_) => Ok(false),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_backend() {
        let backend = McpBackend::new("http://localhost:8080/mcp", 30_000);
        assert_eq!(backend.backend_name(), "mcp");
        assert_eq!(backend.endpoint(), "http://localhost:8080/mcp");
        assert_eq!(backend.timeout_ms(), 30_000);
    }

    #[test]
    fn test_evaluate_request_serialization() {
        let request = EvaluateRequest {
            jsonrpc: "2.0".to_string(),
            method: "rigorix_evaluate_artifact".to_string(),
            params: serde_json::json!({"artifact": {"code": "test"}, "rubric": {"source": {"type": "inline", "content": {}}}}),
            id: 1,
        };
        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("rigorix_evaluate_artifact"));
        assert!(json.contains("2.0"));
    }
}
