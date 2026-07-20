//! MCPBackend — MCP protocol adapter for the Rigorix scoring protocol.
//!
//! @canonical .pi/architecture/modules/scored-evaluation.md#mcp-backend
//! Implements: Contract Freeze — McpBackend stub
//! Issue: #673 (scored-evaluation epic)
//!
//! # Contract (Frozen)
//! - Implements the `ScoringBackend` trait
//! - Sends `rigorix_evaluate_artifact` MCP requests to any server
//!   implementing the Rigorix scoring protocol
//! - Parses MCP responses into `ScoringResult`
//! - Implements health check via `rigorix_ping`
//! - Configurable timeout for MCP requests
//!
//! > **Protocol Ownership:** Rigorix defines the scoring protocol. External
//! > systems like RuntimeAI adopt it by implementing the server side.
//!
//! # Implementation Notes (TODO)
//! - Connect to MCP server via MCP client SDK
//! - Send `rigorix_evaluate_artifact` with artifact + rubric payload
//! - Parse response JSON into `ScoringResult`
//! - Handle timeout, connection errors, invalid responses
//! - Optionally pre-flight with `rigorix_estimate_evaluation_cost`

use async_trait::async_trait;

use crate::scored_evaluation::domain::{
    ScoredEvaluationError, ScoringBackend, ScoringResult, Rubric,
};

/// MCP protocol adapter for the Rigorix scoring protocol.
///
/// Sends `rigorix_evaluate_artifact` requests over MCP to any server
/// that implements the server side of the Rigorix scoring protocol.
pub struct McpBackend {
    /// Name identifier for this backend instance.
    name: &'static str,
    /// MCP server endpoint URL.
    endpoint: String,
    /// Request timeout in milliseconds.
    timeout_ms: u64,
}

impl McpBackend {
    /// Create a new MCP backend adapter.
    pub fn new(endpoint: impl Into<String>, timeout_ms: u64) -> Self {
        Self {
            name: "mcp",
            endpoint: endpoint.into(),
            timeout_ms,
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
        _artifact: &serde_json::Value,
        _rubric: &Rubric,
    ) -> Result<ScoringResult, ScoredEvaluationError> {
        // TODO: implement MCP protocol call
        // 1. Send rigorix_evaluate_artifact request via MCP client
        // 2. Parse response into ScoringResult
        // 3. Handle timeout and error cases
        Err(ScoredEvaluationError::Internal(
            "MCPBackend not yet implemented".to_string(),
        ))
    }

    fn backend_name(&self) -> &'static str {
        self.name
    }

    async fn health_check(&self) -> Result<bool, ScoredEvaluationError> {
        // TODO: implement health check via rigorix_ping
        Err(ScoredEvaluationError::Internal(
            "MCPBackend health check not yet implemented".to_string(),
        ))
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
}
