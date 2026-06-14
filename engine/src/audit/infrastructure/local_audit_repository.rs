//! Local filesystem-based implementation of `AuditEnvelopeRepository`.
//!
//! @canonical .pi/architecture/modules/audit.md
//! Implements: AuditEnvelopeRepository trait — persists envelopes as JSON files
//! Issue: #14
//!
//! Persists audit envelopes as individual JSON files on the local filesystem.
//! Uses atomic write-rename pattern for crash safety. Envelopes are stored
//! under a configurable base path with execution ID as the filename.

use async_trait::async_trait;
use std::path::{Path, PathBuf};

use crate::audit::domain::{AuditEnvelope, AuditError};

use crate::audit::application::dto::RecordDeliveryInput;
use crate::audit::infrastructure::repository::AuditEnvelopeRepository;

/// Filesystem-based implementation of `AuditEnvelopeRepository`.
///
/// Stores envelopes as `{base_path}/{execution_id}.json` files.
/// Uses atomic write-rename pattern for crash-safe persistence.
pub struct LocalAuditEnvelopeRepository {
    /// Base directory for envelope storage.
    base_path: PathBuf,
}

impl LocalAuditEnvelopeRepository {
    /// Create a new repository with the given base path.
    ///
    /// Creates the base directory if it doesn't exist.
    pub fn new(base_path: PathBuf) -> Self {
        std::fs::create_dir_all(&base_path).ok();
        Self { base_path }
    }

    /// Build the file path for an execution ID.
    fn envelope_path(&self, execution_id: &uuid::Uuid) -> PathBuf {
        self.base_path.join(format!("{}.json", execution_id))
    }

    /// Atomically write content to a file using write-rename pattern.
    fn atomic_write(path: &Path, content: &str) -> Result<(), AuditError> {
        let temp_path = path.with_extension("tmp");
        std::fs::write(&temp_path, content).map_err(|e| AuditError::Internal {
            detail: format!("Failed to write temp file: {e}"),
        })?;
        std::fs::rename(&temp_path, path).map_err(|e| AuditError::Internal {
            detail: format!("Failed to rename temp file: {e}"),
        })?;
        Ok(())
    }
}

#[async_trait]
impl AuditEnvelopeRepository for LocalAuditEnvelopeRepository {
    async fn save(&self, envelope: &AuditEnvelope) -> Result<(), AuditError> {
        let json = serde_json::to_string_pretty(envelope).map_err(|e| {
            AuditError::SerializationFailed {
                detail: e.to_string(),
            }
        })?;

        let path = self.envelope_path(&envelope.execution_id);
        Self::atomic_write(&path, &json)?;

        Ok(())
    }

    async fn find_by_execution_id(
        &self,
        execution_id: &uuid::Uuid,
    ) -> Result<Option<AuditEnvelope>, AuditError> {
        let path = self.envelope_path(execution_id);
        if !path.exists() {
            return Ok(None);
        }

        let content = tokio::fs::read_to_string(&path)
            .await
            .map_err(|e| AuditError::Internal {
                detail: format!("Failed to read envelope: {e}"),
            })?;

        let envelope: AuditEnvelope =
            serde_json::from_str(&content).map_err(|e| AuditError::SerializationFailed {
                detail: format!("Failed to deserialize envelope: {e}"),
            })?;

        Ok(Some(envelope))
    }

    async fn list(
        &self,
        since: Option<chrono::DateTime<chrono::Utc>>,
        until: Option<chrono::DateTime<chrono::Utc>>,
        limit: Option<u32>,
    ) -> Result<Vec<AuditEnvelope>, AuditError> {
        let mut envelopes = Vec::new();
        let max = limit.unwrap_or(100) as usize;

        let mut dir =
            tokio::fs::read_dir(&self.base_path)
                .await
                .map_err(|e| AuditError::Internal {
                    detail: format!("Failed to read directory: {e}"),
                })?;

        let mut sorted_entries: Vec<std::path::PathBuf> = Vec::new();
        while let Some(entry) = dir.next_entry().await.map_err(|e| AuditError::Internal {
            detail: format!("Failed to read directory entry: {e}"),
        })? {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "json") {
                sorted_entries.push(path);
            }
        }

        // Sort by modified time (newest first)
        sorted_entries.sort_by(|a, b| {
            let a_time = a.metadata().ok().and_then(|m| m.modified().ok());
            let b_time = b.metadata().ok().and_then(|m| m.modified().ok());
            b_time.cmp(&a_time)
        });

        for path in sorted_entries {
            if envelopes.len() >= max {
                break;
            }

            let content =
                tokio::fs::read_to_string(&path)
                    .await
                    .map_err(|e| AuditError::Internal {
                        detail: format!("Failed to read envelope: {e}"),
                    })?;

            if let Ok(envelope) = serde_json::from_str::<AuditEnvelope>(&content) {
                // Apply date filters
                if let Some(since) = since {
                    if envelope.timestamp < since {
                        continue;
                    }
                }
                if let Some(until) = until {
                    if envelope.timestamp > until {
                        continue;
                    }
                }
                envelopes.push(envelope);
            }
        }

        Ok(envelopes)
    }

    async fn delete(&self, execution_id: &uuid::Uuid) -> Result<(), AuditError> {
        let path = self.envelope_path(execution_id);
        if path.exists() {
            tokio::fs::remove_file(&path)
                .await
                .map_err(|e| AuditError::Internal {
                    detail: format!("Failed to delete envelope: {e}"),
                })?;
        }
        Ok(())
    }

    async fn record_delivery(&self, _input: &RecordDeliveryInput) -> Result<(), AuditError> {
        // Delivery status is tracked externally (queue, circuit breaker).
        // For filesystem storage, the envelope is the source of truth.
        Ok(())
    }

    async fn count(
        &self,
        since: Option<chrono::DateTime<chrono::Utc>>,
        until: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Result<u64, AuditError> {
        let envelopes = self.list(since, until, Some(u32::MAX)).await?;
        Ok(envelopes.len() as u64)
    }

    async fn prune(&self, older_than: chrono::DateTime<chrono::Utc>) -> Result<u64, AuditError> {
        let mut deleted = 0u64;

        let mut dir =
            tokio::fs::read_dir(&self.base_path)
                .await
                .map_err(|e| AuditError::Internal {
                    detail: format!("Failed to read directory: {e}"),
                })?;

        while let Some(entry) = dir.next_entry().await.map_err(|e| AuditError::Internal {
            detail: format!("Failed to read directory entry: {e}"),
        })? {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "json") {
                let content = tokio::fs::read_to_string(&path).await.unwrap_or_default();
                if let Ok(envelope) = serde_json::from_str::<AuditEnvelope>(&content) {
                    if envelope.timestamp < older_than {
                        tokio::fs::remove_file(&path).await.unwrap_or_default();
                        deleted += 1;
                    }
                }
            }
        }

        Ok(deleted)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit::domain::{EventStatus, ExecutionEventRef};
    use tempfile::TempDir;

    fn sample_envelope() -> AuditEnvelope {
        AuditEnvelope {
            execution_id: uuid::Uuid::new_v4(),
            timestamp: chrono::Utc::now(),
            template_id: "test".to_string(),
            planning_hash: "abc123".to_string(),
            events: vec![ExecutionEventRef {
                event_type: "test".to_string(),
                summary: "test event".to_string(),
                occurred_at: chrono::Utc::now(),
                correlation_id: None,
                status: EventStatus::Success,
            }],
            signature: None,
        }
    }

    #[tokio::test]
    async fn test_save_and_find() {
        let dir = TempDir::new().unwrap();
        let repo = LocalAuditEnvelopeRepository::new(dir.path().to_path_buf());

        let envelope = sample_envelope();
        repo.save(&envelope).await.unwrap();

        let found = repo
            .find_by_execution_id(&envelope.execution_id)
            .await
            .unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().execution_id, envelope.execution_id);
    }

    #[tokio::test]
    async fn test_find_not_found() {
        let dir = TempDir::new().unwrap();
        let repo = LocalAuditEnvelopeRepository::new(dir.path().to_path_buf());

        let result = repo
            .find_by_execution_id(&uuid::Uuid::new_v4())
            .await
            .unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_delete() {
        let dir = TempDir::new().unwrap();
        let repo = LocalAuditEnvelopeRepository::new(dir.path().to_path_buf());

        let envelope = sample_envelope();
        repo.save(&envelope).await.unwrap();
        repo.delete(&envelope.execution_id).await.unwrap();

        let result = repo
            .find_by_execution_id(&envelope.execution_id)
            .await
            .unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_list() {
        let dir = TempDir::new().unwrap();
        let repo = LocalAuditEnvelopeRepository::new(dir.path().to_path_buf());

        let e1 = sample_envelope();
        let e2 = sample_envelope();
        repo.save(&e1).await.unwrap();
        repo.save(&e2).await.unwrap();

        let envelopes = repo.list(None, None, None).await.unwrap();
        assert_eq!(envelopes.len(), 2);
    }

    #[tokio::test]
    async fn test_count() {
        let dir = TempDir::new().unwrap();
        let repo = LocalAuditEnvelopeRepository::new(dir.path().to_path_buf());

        repo.save(&sample_envelope()).await.unwrap();
        repo.save(&sample_envelope()).await.unwrap();

        let count = repo.count(None, None).await.unwrap();
        assert_eq!(count, 2);
    }

    #[tokio::test]
    async fn test_prune() {
        let dir = TempDir::new().unwrap();
        let repo = LocalAuditEnvelopeRepository::new(dir.path().to_path_buf());

        // Save two envelopes
        repo.save(&sample_envelope()).await.unwrap();
        repo.save(&sample_envelope()).await.unwrap();

        // Prune everything older than now + 1s (should prune nothing)
        let pruned = repo
            .prune(chrono::Utc::now() + chrono::Duration::seconds(1))
            .await
            .unwrap();
        assert_eq!(pruned, 2); // Both are older than now+1s

        // Prune everything older than now - 1s (should prune nothing)
        let pruned = repo
            .prune(chrono::Utc::now() - chrono::Duration::seconds(1))
            .await
            .unwrap();
        assert_eq!(pruned, 0);
    }
}
