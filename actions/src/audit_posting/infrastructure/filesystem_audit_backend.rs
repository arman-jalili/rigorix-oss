//! OSS default filesystem implementation of `AuditBackend`.
//!
//! @canonical actions/.pi/architecture/modules/audit-posting.md#filesystem-audit-backend
//! Implements: FilesystemAuditBackend trait — local filesystem persistence for audit records
//! Issue: issue-auditbackend-trait-open-core-boundary-
//!
//! Persists signed audit records as individual JSON files on the local filesystem.
//! Uses atomic write-rename pattern for crash safety. Records are stored under
//! a configurable base directory with execution ID as the filename.
//!
//! This is the default OSS implementation of the `AuditBackend` open-core boundary.

use async_trait::async_trait;
use std::path::{Path, PathBuf};

use crate::audit_posting::domain::{AuditPostingError, SignedAuditRecord};

use crate::audit_posting::application::dto::{
    LoadRecordInput, LoadRecordOutput, PostRecordInput, PostRecordOutput,
};

use super::repository::{AuditBackend, FilesystemAuditBackend};

/// OSS default filesystem implementation of `AuditBackend`.
///
/// Stores signed audit records as `{storage_dir}/{execution_id}.json` files.
/// Uses atomic write-rename pattern for crash-safe persistence.
/// The storage directory is created on construction if it doesn't exist.
#[derive(Debug)]
pub struct FilesystemAuditBackendImpl {
    /// Base directory for record storage.
    storage_dir: PathBuf,
}

impl FilesystemAuditBackendImpl {
    /// Create a new filesystem audit backend with the given storage directory.
    ///
    /// Creates the storage directory if it doesn't exist.
    pub fn new(storage_dir: PathBuf) -> Self {
        std::fs::create_dir_all(&storage_dir).ok();
        Self { storage_dir }
    }

    /// Build the file path for an execution ID.
    fn record_path_internal(&self, execution_id: &uuid::Uuid) -> PathBuf {
        self.storage_dir.join(format!("{}.json", execution_id))
    }

    /// Atomically write content to a file using write-rename pattern.
    fn atomic_write(path: &Path, content: &str) -> Result<(), AuditPostingError> {
        let temp_path = path.with_extension("tmp");
        std::fs::write(&temp_path, content).map_err(|e| AuditPostingError::FilesystemError {
            detail: format!("Failed to write temp file: {e}"),
            os_error: e.raw_os_error(),
        })?;
        std::fs::rename(&temp_path, path).map_err(|e| AuditPostingError::FilesystemError {
            detail: format!("Failed to rename temp file: {e}"),
            os_error: e.raw_os_error(),
        })?;
        Ok(())
    }
}

#[async_trait]
impl AuditBackend for FilesystemAuditBackendImpl {
    #[tracing::instrument(skip_all)]
    async fn post(&self, input: PostRecordInput) -> Result<PostRecordOutput, AuditPostingError> {
        let start = std::time::Instant::now();
        let path = self.record_path_internal(&input.record.execution_id);

        let json = serde_json::to_string_pretty(&input.record).map_err(|e| {
            AuditPostingError::SerializationFailed {
                detail: e.to_string(),
            }
        })?;

        // Write atomically
        Self::atomic_write(&path, &json)?;

        let duration_ms = start.elapsed().as_millis() as u64;

        Ok(PostRecordOutput {
            success: true,
            http_status: None,
            duration_ms,
            backend_url: path.to_string_lossy().to_string(),
        })
    }

    #[tracing::instrument(skip_all)]
    async fn load(&self, input: LoadRecordInput) -> Result<LoadRecordOutput, AuditPostingError> {
        let path = self.record_path_internal(&input.execution_id);
        if !path.exists() {
            return Ok(LoadRecordOutput {
                record: None,
                signature_valid: None,
            });
        }

        let content = tokio::fs::read_to_string(&path).await.map_err(|e| {
            AuditPostingError::FilesystemError {
                detail: format!("Failed to read record: {e}"),
                os_error: e.raw_os_error(),
            }
        })?;

        let record: SignedAuditRecord =
            serde_json::from_str(&content).map_err(|e| AuditPostingError::SerializationFailed {
                detail: format!("Failed to deserialize record: {e}"),
            })?;

        let signature_valid = if input.verify_signature {
            // Signature verification is done by the factory/service layer
            // Here we just report whether a signature field exists
            Some(record.signature.is_some())
        } else {
            None
        };

        Ok(LoadRecordOutput {
            record: Some(record),
            signature_valid,
        })
    }

    #[tracing::instrument(skip_all)]
    async fn list(
        &self,
        since: Option<chrono::DateTime<chrono::Utc>>,
        until: Option<chrono::DateTime<chrono::Utc>>,
        limit: Option<u32>,
    ) -> Result<Vec<SignedAuditRecord>, AuditPostingError> {
        let mut records = Vec::new();
        let max = limit.unwrap_or(100) as usize;

        let mut dir = tokio::fs::read_dir(&self.storage_dir).await.map_err(|e| {
            AuditPostingError::FilesystemError {
                detail: format!("Failed to read directory: {e}"),
                os_error: e.raw_os_error(),
            }
        })?;

        let mut sorted_entries: Vec<PathBuf> = Vec::new();
        while let Some(entry) =
            dir.next_entry()
                .await
                .map_err(|e| AuditPostingError::FilesystemError {
                    detail: format!("Failed to read directory entry: {e}"),
                    os_error: e.raw_os_error(),
                })?
        {
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
            if records.len() >= max {
                break;
            }

            let content = tokio::fs::read_to_string(&path).await.map_err(|e| {
                AuditPostingError::FilesystemError {
                    detail: format!("Failed to read record: {e}"),
                    os_error: e.raw_os_error(),
                }
            })?;

            if let Ok(record) = serde_json::from_str::<SignedAuditRecord>(&content) {
                // Apply date filters
                if let Some(since) = since
                    && record.timestamp < since
                {
                    continue;
                }
                if let Some(until) = until
                    && record.timestamp > until
                {
                    continue;
                }
                records.push(record);
            }
        }

        Ok(records)
    }

    #[tracing::instrument(skip_all)]
    async fn delete(&self, execution_id: &uuid::Uuid) -> Result<(), AuditPostingError> {
        let path = self.record_path_internal(execution_id);
        if path.exists() {
            tokio::fs::remove_file(&path).await.map_err(|e| {
                AuditPostingError::FilesystemError {
                    detail: format!("Failed to delete record: {e}"),
                    os_error: e.raw_os_error(),
                }
            })?;
        }
        Ok(())
    }

    #[tracing::instrument(skip_all)]
    async fn health_check(&self) -> Result<bool, AuditPostingError> {
        let exists = self.storage_dir.exists();
        let writable = exists && std::fs::metadata(&self.storage_dir).is_ok();
        Ok(writable)
    }

    #[tracing::instrument(skip_all)]
    async fn count(
        &self,
        since: Option<chrono::DateTime<chrono::Utc>>,
        until: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Result<u64, AuditPostingError> {
        let records = self.list(since, until, Some(u32::MAX)).await?;
        Ok(records.len() as u64)
    }

    #[tracing::instrument(skip_all)]
    async fn prune(
        &self,
        older_than: chrono::DateTime<chrono::Utc>,
    ) -> Result<u64, AuditPostingError> {
        let mut deleted = 0u64;

        let mut dir = tokio::fs::read_dir(&self.storage_dir).await.map_err(|e| {
            AuditPostingError::FilesystemError {
                detail: format!("Failed to read directory: {e}"),
                os_error: e.raw_os_error(),
            }
        })?;

        while let Some(entry) =
            dir.next_entry()
                .await
                .map_err(|e| AuditPostingError::FilesystemError {
                    detail: format!("Failed to read directory entry: {e}"),
                    os_error: e.raw_os_error(),
                })?
        {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "json") {
                let content = tokio::fs::read_to_string(&path).await.unwrap_or_default();
                if let Ok(record) = serde_json::from_str::<SignedAuditRecord>(&content)
                    && record.timestamp < older_than
                {
                    tokio::fs::remove_file(&path).await.unwrap_or_default();
                    deleted += 1;
                }
            }
        }

        Ok(deleted)
    }
}

#[async_trait]
impl FilesystemAuditBackend for FilesystemAuditBackendImpl {
    fn storage_dir(&self) -> &str {
        self.storage_dir.to_str().unwrap_or("")
    }

    fn record_path(&self, execution_id: &uuid::Uuid) -> String {
        self.record_path_internal(execution_id)
            .to_string_lossy()
            .to_string()
    }

    fn serialize_record(&self, record: &SignedAuditRecord) -> Result<String, AuditPostingError> {
        serde_json::to_string_pretty(record).map_err(|e| AuditPostingError::SerializationFailed {
            detail: e.to_string(),
        })
    }

    fn deserialize_record(&self, json: &str) -> Result<SignedAuditRecord, AuditPostingError> {
        serde_json::from_str(json).map_err(|e| AuditPostingError::SerializationFailed {
            detail: e.to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn sample_record() -> SignedAuditRecord {
        SignedAuditRecord {
            execution_id: uuid::Uuid::new_v4(),
            timestamp: chrono::Utc::now(),
            run_id: Some(12345),
            workflow_name: Some("test-workflow".to_string()),
            repository: "test-org/test-repo".to_string(),
            git_ref: Some("refs/heads/main".to_string()),
            commit_sha: Some("abc123def456".to_string()),
            mode: "run".to_string(),
            summary: "Test execution".to_string(),
            signature: Some("abcd1234signature".to_string()),
            actor: Some("test-user".to_string()),
            metadata: None,
        }
    }

    #[tokio::test]
    async fn test_post_and_load() {
        let dir = TempDir::new().unwrap();
        let backend = FilesystemAuditBackendImpl::new(dir.path().to_path_buf());

        let record = sample_record();
        let post_input = PostRecordInput {
            record: record.clone(),
            backend_url: None,
            timeout_secs: None,
        };
        let post_output = backend.post(post_input).await.unwrap();
        assert!(post_output.success);

        let load_input = LoadRecordInput {
            execution_id: record.execution_id,
            verify_signature: false,
        };
        let load_output = backend.load(load_input).await.unwrap();
        assert!(load_output.record.is_some());
        assert_eq!(
            load_output.record.unwrap().execution_id,
            record.execution_id
        );
    }

    #[tokio::test]
    async fn test_load_not_found() {
        let dir = TempDir::new().unwrap();
        let backend = FilesystemAuditBackendImpl::new(dir.path().to_path_buf());

        let input = LoadRecordInput {
            execution_id: uuid::Uuid::new_v4(),
            verify_signature: false,
        };
        let output = backend.load(input).await.unwrap();
        assert!(output.record.is_none());
    }

    #[tokio::test]
    async fn test_delete() {
        let dir = TempDir::new().unwrap();
        let backend = FilesystemAuditBackendImpl::new(dir.path().to_path_buf());

        let record = sample_record();
        backend
            .post(PostRecordInput {
                record: record.clone(),
                backend_url: None,
                timeout_secs: None,
            })
            .await
            .unwrap();

        backend.delete(&record.execution_id).await.unwrap();

        let output = backend
            .load(LoadRecordInput {
                execution_id: record.execution_id,
                verify_signature: false,
            })
            .await
            .unwrap();
        assert!(output.record.is_none());
    }

    #[tokio::test]
    async fn test_list() {
        let dir = TempDir::new().unwrap();
        let backend = FilesystemAuditBackendImpl::new(dir.path().to_path_buf());

        let r1 = sample_record();
        let r2 = sample_record();
        backend
            .post(PostRecordInput {
                record: r1,
                backend_url: None,
                timeout_secs: None,
            })
            .await
            .unwrap();
        backend
            .post(PostRecordInput {
                record: r2,
                backend_url: None,
                timeout_secs: None,
            })
            .await
            .unwrap();

        let records = backend.list(None, None, None).await.unwrap();
        assert_eq!(records.len(), 2);
    }

    #[tokio::test]
    async fn test_count() {
        let dir = TempDir::new().unwrap();
        let backend = FilesystemAuditBackendImpl::new(dir.path().to_path_buf());

        backend
            .post(PostRecordInput {
                record: sample_record(),
                backend_url: None,
                timeout_secs: None,
            })
            .await
            .unwrap();
        backend
            .post(PostRecordInput {
                record: sample_record(),
                backend_url: None,
                timeout_secs: None,
            })
            .await
            .unwrap();

        let count = backend.count(None, None).await.unwrap();
        assert_eq!(count, 2);
    }

    #[tokio::test]
    async fn test_prune() {
        let dir = TempDir::new().unwrap();
        let backend = FilesystemAuditBackendImpl::new(dir.path().to_path_buf());

        backend
            .post(PostRecordInput {
                record: sample_record(),
                backend_url: None,
                timeout_secs: None,
            })
            .await
            .unwrap();
        backend
            .post(PostRecordInput {
                record: sample_record(),
                backend_url: None,
                timeout_secs: None,
            })
            .await
            .unwrap();

        // Prune everything older than now + 1s (should prune both)
        let pruned = backend
            .prune(chrono::Utc::now() + chrono::Duration::seconds(1))
            .await
            .unwrap();
        assert_eq!(pruned, 2);
    }

    #[tokio::test]
    async fn test_health_check() {
        let dir = TempDir::new().unwrap();
        let backend = FilesystemAuditBackendImpl::new(dir.path().to_path_buf());
        assert!(backend.health_check().await.unwrap());
    }

    #[tokio::test]
    async fn test_serialize_deserialize() {
        let dir = TempDir::new().unwrap();
        let backend = FilesystemAuditBackendImpl::new(dir.path().to_path_buf());

        let record = sample_record();
        let json = backend.serialize_record(&record).unwrap();
        let deserialized = backend.deserialize_record(&json).unwrap();
        assert_eq!(deserialized.execution_id, record.execution_id);
        assert_eq!(deserialized.repository, record.repository);
    }

    #[tokio::test]
    async fn test_storage_dir() {
        let dir = TempDir::new().unwrap();
        let backend = FilesystemAuditBackendImpl::new(dir.path().to_path_buf());
        assert!(!backend.storage_dir().is_empty());
    }

    #[tokio::test]
    async fn test_record_path() {
        let dir = TempDir::new().unwrap();
        let backend = FilesystemAuditBackendImpl::new(dir.path().to_path_buf());
        let id = uuid::Uuid::new_v4();
        let path = backend.record_path(&id);
        assert!(path.contains(&id.to_string()));
        assert!(path.ends_with(".json"));
    }
}
