//! LocalEvaluationRepository — filesystem-backed evaluation result persistence.
//!
//! @canonical .pi/architecture/modules/scored-evaluation.md#evaluation-repository
//! Implements: Filesystem-backed EvaluationRepository
//! Issue: #677 (scored-evaluation epic)
//!
//! Persists evaluation results as JSON files on the filesystem using
//! atomic write-rename pattern for crash safety.

use async_trait::async_trait;
use uuid::Uuid;

use crate::scored_evaluation::application::dto::EvaluateOutput;
use crate::scored_evaluation::domain::ScoredEvaluationError;

use super::EvaluationRepository;

/// Filesystem-backed implementation of EvaluationRepository.
///
/// Stores evaluation results as JSON files under a configurable
/// base directory, organized by execution ID:
/// `{base_dir}/{execution_id}/{node_id}.json`
pub struct LocalEvaluationRepository {
    base_dir: std::path::PathBuf,
}

impl LocalEvaluationRepository {
    /// Create a new filesystem-backed repository.
    ///
    /// Results are stored under `base_dir` (default: `.rigorix/evaluations/`).
    pub fn new(base_dir: impl Into<std::path::PathBuf>) -> Self {
        Self {
            base_dir: base_dir.into(),
        }
    }

    /// Get the file path for a specific evaluation result.
    fn result_path(&self, execution_id: Uuid, node_id: Uuid) -> std::path::PathBuf {
        let exec_dir = self.base_dir.join(execution_id.to_string());
        exec_dir.join(format!("{}.json", node_id))
    }

    /// Get the directory for an execution.
    fn execution_dir(&self, execution_id: Uuid) -> std::path::PathBuf {
        self.base_dir.join(execution_id.to_string())
    }

    /// Ensure the execution directory exists.
    async fn ensure_dir(&self, execution_id: Uuid) -> Result<(), ScoredEvaluationError> {
        let dir = self.execution_dir(execution_id);
        tokio::fs::create_dir_all(&dir).await.map_err(|e| {
            ScoredEvaluationError::Internal(format!("Failed to create directory: {}", e))
        })
    }
}

#[async_trait]
impl EvaluationRepository for LocalEvaluationRepository {
    async fn save(&self, output: &EvaluateOutput) -> Result<(), ScoredEvaluationError> {
        self.ensure_dir(output.execution_id).await?;

        let path = self.result_path(output.execution_id, output.node_id);
        let tmp_path = path.with_extension("tmp");

        let json = serde_json::to_string_pretty(output)
            .map_err(|e| ScoredEvaluationError::Internal(format!("Serialization error: {}", e)))?;

        // Atomic write: write to .tmp, then rename
        tokio::fs::write(&tmp_path, &json)
            .await
            .map_err(|e| ScoredEvaluationError::Internal(format!("Write error: {}", e)))?;

        tokio::fs::rename(&tmp_path, &path)
            .await
            .map_err(|e| ScoredEvaluationError::Internal(format!("Rename error: {}", e)))?;

        Ok(())
    }

    async fn get(
        &self,
        execution_id: Uuid,
        node_id: Uuid,
    ) -> Result<Option<EvaluateOutput>, ScoredEvaluationError> {
        let path = self.result_path(execution_id, node_id);

        match tokio::fs::read_to_string(&path).await {
            Ok(json) => {
                let output: EvaluateOutput = serde_json::from_str(&json).map_err(|e| {
                    ScoredEvaluationError::Internal(format!("Deserialization error: {}", e))
                })?;
                Ok(Some(output))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(ScoredEvaluationError::Internal(format!(
                "Read error: {}",
                e
            ))),
        }
    }

    async fn list(&self, execution_id: Uuid) -> Result<Vec<EvaluateOutput>, ScoredEvaluationError> {
        let dir = self.execution_dir(execution_id);
        let mut results = Vec::new();

        let mut entries = match tokio::fs::read_dir(&dir).await {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(results),
            Err(e) => {
                return Err(ScoredEvaluationError::Internal(format!(
                    "List error: {}",
                    e
                )));
            }
        };

        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| ScoredEvaluationError::Internal(format!("Read dir error: {}", e)))?
        {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "json")
                && let Ok(json) = tokio::fs::read_to_string(&path).await
                    && let Ok(output) = serde_json::from_str::<EvaluateOutput>(&json) {
                        results.push(output);
                    }
        }

        Ok(results)
    }

    async fn delete_by_execution(&self, execution_id: Uuid) -> Result<(), ScoredEvaluationError> {
        let dir = self.execution_dir(execution_id);
        match tokio::fs::remove_dir_all(&dir).await {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(ScoredEvaluationError::Internal(format!(
                "Delete error: {}",
                e
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scored_evaluation::domain::{ScoreDimension, ScoringResult};
    use chrono::Utc;
    use std::collections::HashMap;
    use tempfile::TempDir;

    fn make_output() -> EvaluateOutput {
        let mut dims = HashMap::new();
        dims.insert(
            "correctness".to_string(),
            ScoreDimension::new(0.95, 1.0, "Correctness", true),
        );
        let result = ScoringResult::new(true, dims, "ok", "test", 100, None);
        EvaluateOutput::new(result, Uuid::new_v4(), Uuid::new_v4(), "test", Utc::now())
    }

    #[tokio::test]
    async fn test_save_and_get() {
        let dir = TempDir::new().unwrap();
        let repo = LocalEvaluationRepository::new(dir.path());
        let output = make_output();

        repo.save(&output).await.unwrap();
        let retrieved = repo.get(output.execution_id, output.node_id).await.unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().node_name, output.node_name);
    }

    #[tokio::test]
    async fn test_get_not_found() {
        let dir = TempDir::new().unwrap();
        let repo = LocalEvaluationRepository::new(dir.path());
        let result = repo.get(Uuid::new_v4(), Uuid::new_v4()).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_list() {
        let dir = TempDir::new().unwrap();
        let repo = LocalEvaluationRepository::new(dir.path());
        let output = make_output();
        repo.save(&output).await.unwrap();

        let results = repo.list(output.execution_id).await.unwrap();
        assert_eq!(results.len(), 1);
    }

    #[tokio::test]
    async fn test_delete_by_execution() {
        let dir = TempDir::new().unwrap();
        let repo = LocalEvaluationRepository::new(dir.path());
        let output = make_output();
        repo.save(&output).await.unwrap();

        repo.delete_by_execution(output.execution_id).await.unwrap();
        let results = repo.list(output.execution_id).await.unwrap();
        assert!(results.is_empty());
    }
}
