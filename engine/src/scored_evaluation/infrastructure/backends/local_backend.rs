//! LocalBackend — local script adapter for the Rigorix scoring protocol.
//!
//! @canonical .pi/architecture/modules/scored-evaluation.md#local-backend
//! Implements: Contract Freeze — LocalBackend stub
//! Issue: #673 (scored-evaluation epic)
//!
//! # Contract (Frozen)
//! - Implements the `ScoringBackend` trait
//! - Executes a local script with artifact + rubric as environment variables
//! - Reads scoring result from stdout (JSON)
//! - Configurable script path and timeout
//! - Implements health check by checking script file existence
//!
//! # Implementation Notes (TODO)
//! - Execute script via tokio::process::Command
//! - Pass artifact and rubric as env vars (or stdin JSON)
//! - Parse stdout JSON into ScoringResult
//! - Validate script path against allowlist for security
//! - Configurable timeout for script execution

use async_trait::async_trait;

use crate::scored_evaluation::domain::{
    ScoredEvaluationError, ScoringBackend, ScoringResult, Rubric,
};

/// Local script adapter for the Rigorix scoring protocol.
///
/// Executes a configurable local script that reads an artifact + rubric
/// from environment variables or stdin and outputs a `ScoringResult` JSON
/// to stdout.
pub struct LocalBackend {
    /// Name identifier for this backend instance.
    name: &'static str,
    /// Path to the scoring script.
    script_path: String,
    /// Execution timeout in milliseconds.
    timeout_ms: u64,
}

impl LocalBackend {
    /// Create a new local backend adapter.
    pub fn new(script_path: impl Into<String>, timeout_ms: u64) -> Self {
        Self {
            name: "local",
            script_path: script_path.into(),
            timeout_ms,
        }
    }

    /// Returns the script path.
    pub fn script_path(&self) -> &str {
        &self.script_path
    }

    /// Returns the timeout in milliseconds.
    pub fn timeout_ms(&self) -> u64 {
        self.timeout_ms
    }
}

#[async_trait]
impl ScoringBackend for LocalBackend {
    async fn evaluate(
        &self,
        _artifact: &serde_json::Value,
        _rubric: &Rubric,
    ) -> Result<ScoringResult, ScoredEvaluationError> {
        // TODO: implement local script execution
        // 1. Validate script path against allowlist
        // 2. Execute script with artifact + rubric as env vars
        // 3. Read stdout, parse as ScoringResult
        // 4. Handle timeout, non-zero exit, invalid output
        Err(ScoredEvaluationError::Internal(
            "LocalBackend not yet implemented".to_string(),
        ))
    }

    fn backend_name(&self) -> &'static str {
        self.name
    }

    async fn health_check(&self) -> Result<bool, ScoredEvaluationError> {
        // TODO: check that script file exists and is executable
        // let metadata = tokio::fs::metadata(&self.script_path).await;
        // Ok(metadata.is_ok())
        Err(ScoredEvaluationError::Internal(
            "LocalBackend health check not yet implemented".to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_backend() {
        let backend = LocalBackend::new("./scripts/evaluate.sh", 10_000);
        assert_eq!(backend.backend_name(), "local");
        assert_eq!(backend.script_path(), "./scripts/evaluate.sh");
        assert_eq!(backend.timeout_ms(), 10_000);
    }
}
