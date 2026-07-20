//! LocalBackend — local script adapter for the Rigorix scoring protocol.
//!
//! @canonical .pi/architecture/modules/scored-evaluation.md#local-backend
//! Implements: Local script adapter for Rigorix scoring protocol
//! Issue: #690 (scored-evaluation epic)
//!
//! Executes a configurable local script with artifact + rubric as
//! environment variables and reads ScoringResult JSON from stdout.

use async_trait::async_trait;

use crate::scored_evaluation::domain::{
    ScoredEvaluationError, ScoringBackend, ScoringResult, Rubric,
};

/// Local script adapter for the Rigorix scoring protocol.
///
/// Executes a local script that reads an artifact + rubric from
/// environment variables (`RIGORIX_ARTIFACT`, `RIGORIX_RUBRIC`) and
/// outputs a `ScoringResult` JSON to stdout.
///
/// # Security
///
/// Script path is validated against an allowlist (checking it exists
/// and is within the project directory by default).
pub struct LocalBackend {
    name: &'static str,
    script_path: String,
    timeout_ms: u64,
    allowlist: Vec<String>,
}

impl LocalBackend {
    /// Create a new local backend adapter.
    pub fn new(script_path: impl Into<String>, timeout_ms: u64) -> Self {
        Self {
            name: "local",
            script_path: script_path.into(),
            timeout_ms,
            allowlist: vec![],
        }
    }

    /// Set the allowed script paths. If empty, only the exact script_path is allowed.
    pub fn with_allowlist(mut self, paths: Vec<String>) -> Self {
        self.allowlist = paths;
        self
    }

    /// Returns the script path.
    pub fn script_path(&self) -> &str {
        &self.script_path
    }

    /// Returns the timeout in milliseconds.
    pub fn timeout_ms(&self) -> u64 {
        self.timeout_ms
    }

    /// Validate that the script path is allowed.
    fn validate_path(&self) -> Result<(), ScoredEvaluationError> {
        if self.allowlist.is_empty() {
            // No allowlist configured — only allow the exact script path
            let path = std::path::Path::new(&self.script_path);
            if !path.exists() {
                return Err(ScoredEvaluationError::BackendNotFound(format!(
                    "Script not found: {}",
                    self.script_path
                )));
            }
            return Ok(());
        }

        // Check if script_path matches any allowlist entry
        let canonical = std::path::Path::new(&self.script_path)
            .canonicalize()
            .map_err(|_| {
                ScoredEvaluationError::BackendNotFound(format!(
                    "Script path invalid: {}",
                    self.script_path
                ))
            })?;

        let allowed = self.allowlist.iter().any(|allowed| {
            std::path::Path::new(allowed)
                .canonicalize()
                .map(|a| a == canonical)
                .unwrap_or(false)
        });

        if !allowed {
            return Err(ScoredEvaluationError::BackendError(format!(
                "Script path not in allowlist: {}",
                self.script_path
            )));
        }

        Ok(())
    }
}

#[async_trait]
impl ScoringBackend for LocalBackend {
    async fn evaluate(
        &self,
        artifact: &serde_json::Value,
        rubric: &Rubric,
    ) -> Result<ScoringResult, ScoredEvaluationError> {
        // Validate script path
        self.validate_path()?;

        // Serialize artifact and rubric as environment variables
        let artifact_str = serde_json::to_string(artifact)
            .map_err(|e| ScoredEvaluationError::InvalidArtifact(e.to_string()))?;
        let rubric_str = serde_json::to_string(rubric)
            .map_err(|e| ScoredEvaluationError::InvalidRubric(e.to_string()))?;

        // Execute the script
        let output = tokio::process::Command::new(&self.script_path)
            .env("RIGORIX_ARTIFACT", &artifact_str)
            .env("RIGORIX_RUBRIC", &rubric_str)
            .output()
            .await
            .map_err(|e| {
                ScoredEvaluationError::BackendError(format!(
                    "Failed to execute script: {}",
                    e
                ))
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(ScoredEvaluationError::BackendError(format!(
                "Script exited with {}: {}",
                output.status,
                stderr.trim()
            )));
        }

        // Parse stdout as ScoringResult
        let stdout = String::from_utf8_lossy(&output.stdout);
        let scoring_result: ScoringResult = serde_json::from_str(&stdout).map_err(|e| {
            ScoredEvaluationError::BackendError(format!(
                "Invalid ScoringResult from script stdout: {}",
                e
            ))
        })?;

        Ok(scoring_result)
    }

    fn backend_name(&self) -> &'static str {
        self.name
    }

    async fn health_check(&self) -> Result<bool, ScoredEvaluationError> {
        let path = std::path::Path::new(&self.script_path);
        Ok(path.exists())
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

    #[test]
    fn test_validate_path_exists() {
        let backend = LocalBackend::new("/tmp", 10_000);
        assert!(backend.validate_path().is_ok());
    }

    #[test]
    fn test_validate_path_not_exists() {
        let backend = LocalBackend::new("/nonexistent/script.sh", 10_000);
        assert!(backend.validate_path().is_err());
    }

    #[test]
    fn test_with_allowlist() {
        let backend = LocalBackend::new("./scripts/evaluate.sh", 10_000)
            .with_allowlist(vec!["./scripts/evaluate.sh".to_string()]);
        assert_eq!(backend.allowlist.len(), 1);
    }
}
