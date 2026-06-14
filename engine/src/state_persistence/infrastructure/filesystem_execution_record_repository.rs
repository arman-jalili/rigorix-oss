//! Filesystem implementation of `ExecutionRecordRepository`.
//!
//! @canonical .pi/architecture/modules/state-persistence.md#infrastructure
//! Implements: ISSUE-STATE-PERSISTENCE-2 — FileSystemExecutionRecordRepository
//! Issue: #80
//!
//! Provides a filesystem-backed `ExecutionRecordRepository` that stores
//! complete execution records (state + events + graph) as JSON files.

use async_trait::async_trait;
use std::path::PathBuf;
use tokio::fs;
use uuid::Uuid;

use crate::state_persistence::domain::{ExecutionRecord, StateError};

use super::repository::ExecutionRecordRepository;

/// Filesystem-backed implementation of `ExecutionRecordRepository`.
///
/// Stores execution records as `{record_dir}/{record_id}.record.json`.
pub struct FileSystemExecutionRecordRepository {
    record_dir: PathBuf,
}

impl FileSystemExecutionRecordRepository {
    pub async fn new(record_dir: impl Into<PathBuf>) -> Result<Self, StateError> {
        let record_dir: PathBuf = record_dir.into();

        if !record_dir.exists() {
            fs::create_dir_all(&record_dir)
                .await
                .map_err(|e| StateError::DirectoryError {
                    detail: format!("Failed to create record directory {:?}: {}", record_dir, e),
                })?;
        }

        if !record_dir.is_dir() {
            return Err(StateError::DirectoryError {
                detail: format!("Record path {:?} exists but is not a directory", record_dir),
            });
        }

        Ok(Self { record_dir })
    }

    fn record_path(&self, record_id: Uuid) -> PathBuf {
        self.record_dir.join(format!("{}.record.json", record_id))
    }

    fn execution_index_path(&self, execution_id: Uuid) -> PathBuf {
        self.record_dir
            .join(format!("idx_{}.record.json", execution_id))
    }
}

#[async_trait]
impl ExecutionRecordRepository for FileSystemExecutionRecordRepository {
    async fn save_record(&self, record: &ExecutionRecord) -> Result<(), StateError> {
        let path = self.record_path(record.record_id);
        let temp = self
            .record_dir
            .join(format!("{}.record.json.tmp", record.record_id));

        let json =
            serde_json::to_string_pretty(record).map_err(|e| StateError::SerialisationError {
                detail: format!("Failed to serialise execution record: {}", e),
            })?;

        fs::write(&temp, &json)
            .await
            .map_err(|e| StateError::IoError {
                detail: format!("Failed to write temp record file: {}", e),
            })?;

        fs::rename(&temp, &path)
            .await
            .map_err(|e| StateError::IoError {
                detail: format!("Failed to rename record file: {}", e),
            })?;

        // Create execution_id index
        let idx_path = self.execution_index_path(record.execution_id);
        if !idx_path.exists() {
            fs::copy(&path, &idx_path)
                .await
                .map_err(|e| StateError::IoError {
                    detail: format!("Failed to create record execution index: {}", e),
                })?;
        }

        Ok(())
    }

    async fn load_record(&self, record_id: Uuid) -> Result<ExecutionRecord, StateError> {
        let path = self.record_path(record_id);
        if !path.exists() {
            return Err(StateError::StateNotFound {
                execution_id: record_id.to_string(),
            });
        }

        let data = fs::read_to_string(&path)
            .await
            .map_err(|e| StateError::IoError {
                detail: format!("Failed to read record file: {}", e),
            })?;

        serde_json::from_str(&data).map_err(|e| StateError::CorruptedState {
            path: path.to_string_lossy().to_string(),
            detail: format!("Failed to deserialise record: {}", e),
        })
    }

    async fn load_by_execution_id(
        &self,
        execution_id: Uuid,
    ) -> Result<ExecutionRecord, StateError> {
        let idx_path = self.execution_index_path(execution_id);
        if !idx_path.exists() {
            return Err(StateError::StateNotFound {
                execution_id: format!("execution:{}", execution_id),
            });
        }

        let data = fs::read_to_string(&idx_path)
            .await
            .map_err(|e| StateError::IoError {
                detail: format!("Failed to read record index file: {}", e),
            })?;

        serde_json::from_str(&data).map_err(|e| StateError::CorruptedState {
            path: idx_path.to_string_lossy().to_string(),
            detail: format!("Failed to deserialise record from index: {}", e),
        })
    }

    async fn delete_record(&self, record_id: Uuid) -> Result<(), StateError> {
        let path = self.record_path(record_id);
        let temp = self
            .record_dir
            .join(format!("{}.record.json.tmp", record_id));

        if path.exists() {
            if let Ok(record) = self.load_record(record_id).await {
                let idx_path = self.execution_index_path(record.execution_id);
                if idx_path.exists() {
                    let _ = fs::remove_file(&idx_path).await;
                }
            }
            fs::remove_file(&path)
                .await
                .map_err(|e| StateError::IoError {
                    detail: format!("Failed to delete record file: {}", e),
                })?;
        }

        if temp.exists() {
            let _ = fs::remove_file(&temp).await;
        }

        Ok(())
    }

    async fn list_records(&self, limit: u32, offset: u32) -> Result<Vec<Uuid>, StateError> {
        let mut entries =
            fs::read_dir(&self.record_dir)
                .await
                .map_err(|e| StateError::IoError {
                    detail: format!("Failed to read record directory: {}", e),
                })?;

        let mut ids = Vec::new();
        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| StateError::IoError {
                detail: format!("Failed to read directory entry: {}", e),
            })?
        {
            let file_name = entry.file_name().to_string_lossy().to_string();
            if !file_name.ends_with(".record.json") || file_name.starts_with("idx_") {
                continue;
            }
            if let Some(stem) = file_name.strip_suffix(".record.json") {
                if let Ok(uuid) = Uuid::parse_str(stem) {
                    ids.push(uuid);
                }
            }
        }

        ids.sort();
        let offset = offset as usize;
        let limit = limit as usize;
        if offset >= ids.len() {
            return Ok(Vec::new());
        }
        Ok(ids[offset..std::cmp::min(offset + limit, ids.len())].to_vec())
    }

    async fn count(&self) -> Result<u64, StateError> {
        Ok(self.list_records(u32::MAX, 0).await?.len() as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use tempfile::TempDir;

    use crate::state_persistence::domain::{ExecutionGraph, ExecutionGraphNode, ExecutionStatus};

    fn create_test_record() -> ExecutionRecord {
        let execution_id = Uuid::new_v4();
        let graph = ExecutionGraph::new(
            execution_id,
            "test".to_string(),
            ExecutionStatus::Completed,
            Utc::now(),
            Some(Utc::now()),
            vec![ExecutionGraphNode::new(
                Uuid::new_v4(),
                "node1".to_string(),
                "build".to_string(),
                vec![],
            )],
            1000,
        );

        ExecutionRecord::new(
            execution_id,
            "test-execution".to_string(),
            ExecutionStatus::Completed,
            Utc::now(),
            Some(Utc::now()),
            1000,
            "hash123".to_string(),
            5,
            1,
            1,
            0,
            0,
            graph,
        )
    }

    async fn create_repo() -> (FileSystemExecutionRecordRepository, TempDir) {
        let dir = TempDir::new().unwrap();
        let repo = FileSystemExecutionRecordRepository::new(dir.path().to_path_buf())
            .await
            .unwrap();
        (repo, dir)
    }

    #[tokio::test]
    async fn test_save_and_load_record() {
        let (repo, _dir) = create_repo().await;
        let record = create_test_record();

        repo.save_record(&record).await.unwrap();
        let loaded = repo.load_record(record.record_id).await.unwrap();

        assert_eq!(loaded.record_id, record.record_id);
        assert_eq!(loaded.execution_id, record.execution_id);
        assert_eq!(loaded.name, "test-execution");
        assert_eq!(loaded.status, ExecutionStatus::Completed);
    }

    #[tokio::test]
    async fn test_load_by_execution_id() {
        let (repo, _dir) = create_repo().await;
        let record = create_test_record();

        repo.save_record(&record).await.unwrap();
        let loaded = repo
            .load_by_execution_id(record.execution_id)
            .await
            .unwrap();
        assert_eq!(loaded.execution_id, record.execution_id);
    }

    #[tokio::test]
    async fn test_delete_record() {
        let (repo, _dir) = create_repo().await;
        let record = create_test_record();

        repo.save_record(&record).await.unwrap();
        repo.delete_record(record.record_id).await.unwrap();

        assert!(matches!(
            repo.load_record(record.record_id).await,
            Err(StateError::StateNotFound { .. })
        ));
    }

    #[tokio::test]
    async fn test_list_records() {
        let (repo, _dir) = create_repo().await;
        assert_eq!(repo.list_records(10, 0).await.unwrap().len(), 0);

        repo.save_record(&create_test_record()).await.unwrap();
        repo.save_record(&create_test_record()).await.unwrap();
        assert_eq!(repo.list_records(10, 0).await.unwrap().len(), 2);
    }

    #[tokio::test]
    async fn test_count() {
        let (repo, _dir) = create_repo().await;
        assert_eq!(repo.count().await.unwrap(), 0);

        repo.save_record(&create_test_record()).await.unwrap();
        assert_eq!(repo.count().await.unwrap(), 1);
    }
}
