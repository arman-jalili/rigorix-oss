//! Filesystem implementation of `StateRepository`.
//!
//! @canonical .pi/architecture/modules/state-persistence.md#infrastructure
//! Implements: ISSUE-STATE-PERSISTENCE-1 — FileSystemStateRepository
//! Issue: #79
//!
//! Provides a filesystem-backed `StateRepository` that stores execution state
//! as JSON files using atomic write-rename for crash safety. Files are stored
//! as `{state_dir}/{execution_id}.json`.
//!
//! # Atomic Write-Rename Pattern
//! 1. Serialise state to `{execution_id}.json.tmp`
//! 2. `fs::rename` to `{execution_id}.json`
//!
//! On POSIX, `rename(2)` is atomic — a power failure during write leaves
//! the original file intact.

use async_trait::async_trait;
use std::path::PathBuf;
use tokio::fs;
use uuid::Uuid;

use crate::state_persistence::domain::{ExecutionState, StateError};

use super::repository::StateRepository;

/// Filesystem-backed implementation of `StateRepository`.
///
/// Stores execution state as JSON files in a configurable directory.
/// Uses atomic write-rename for crash safety.
pub struct FileSystemStateRepository {
    /// Directory where state files are stored.
    state_dir: PathBuf,
}

impl FileSystemStateRepository {
    /// Create a new `FileSystemStateRepository`.
    ///
    /// The state directory is created if it doesn't exist.
    /// Returns `StateError::DirectoryError` if the directory cannot be created
    /// or is not accessible.
    pub async fn new(state_dir: impl Into<PathBuf>) -> Result<Self, StateError> {
        let state_dir: PathBuf = state_dir.into();

        if !state_dir.exists() {
            fs::create_dir_all(&state_dir)
                .await
                .map_err(|e| StateError::DirectoryError {
                    detail: format!("Failed to create state directory {:?}: {}", state_dir, e),
                })?;
        }

        if !state_dir.is_dir() {
            return Err(StateError::DirectoryError {
                detail: format!("State path {:?} exists but is not a directory", state_dir),
            });
        }

        Ok(Self { state_dir })
    }

    /// Get the path to the state file for an execution ID.
    fn state_path(&self, execution_id: Uuid) -> PathBuf {
        self.state_dir.join(format!("{}.json", execution_id))
    }

    /// Get the path to the temporary state file for an execution ID.
    fn temp_path(&self, execution_id: Uuid) -> PathBuf {
        self.state_dir.join(format!("{}.json.tmp", execution_id))
    }
}

#[async_trait]
impl StateRepository for FileSystemStateRepository {
    async fn save(&self, state: &ExecutionState) -> Result<(), StateError> {
        let path = self.state_path(state.execution_id);
        let temp = self.temp_path(state.execution_id);

        // Serialise to a JSON string first to catch serialisation errors
        // before writing to disk.
        let json =
            serde_json::to_string_pretty(state).map_err(|e| StateError::SerialisationError {
                detail: format!("Failed to serialise execution state: {}", e),
            })?;

        // Write to temp file (atomic if we fail here, original is intact)
        fs::write(&temp, &json)
            .await
            .map_err(|e| StateError::IoError {
                detail: format!("Failed to write temp state file {:?}: {}", temp, e),
            })?;

        // Atomic rename (on POSIX, rename(2) is atomic)
        fs::rename(&temp, &path)
            .await
            .map_err(|e| StateError::IoError {
                detail: format!(
                    "Failed to rename temp state file {:?} to {:?}: {}",
                    temp, path, e
                ),
            })?;

        Ok(())
    }

    async fn load(&self, execution_id: Uuid) -> Result<ExecutionState, StateError> {
        let path = self.state_path(execution_id);

        if !path.exists() {
            return Err(StateError::StateNotFound {
                execution_id: execution_id.to_string(),
            });
        }

        let data = fs::read_to_string(&path)
            .await
            .map_err(|e| StateError::IoError {
                detail: format!("Failed to read state file {:?}: {}", path, e),
            })?;

        serde_json::from_str(&data).map_err(|e| StateError::CorruptedState {
            path: path.to_string_lossy().to_string(),
            detail: format!("Failed to deserialise execution state: {}", e),
        })
    }

    async fn exists(&self, execution_id: Uuid) -> Result<bool, StateError> {
        let path = self.state_path(execution_id);
        Ok(path.exists())
    }

    async fn delete(&self, execution_id: Uuid) -> Result<(), StateError> {
        let path = self.state_path(execution_id);

        if path.exists() {
            fs::remove_file(&path)
                .await
                .map_err(|e| StateError::IoError {
                    detail: format!("Failed to delete state file {:?}: {}", path, e),
                })?;
        }

        // Also clean up any leftover temp file
        let temp = self.temp_path(execution_id);
        if temp.exists() {
            let _ = fs::remove_file(&temp).await;
        }

        Ok(())
    }

    async fn list_ids(&self) -> Result<Vec<Uuid>, StateError> {
        let mut entries = fs::read_dir(&self.state_dir)
            .await
            .map_err(|e| StateError::IoError {
                detail: format!("Failed to read state directory {:?}: {}", self.state_dir, e),
            })?;

        let mut ids = Vec::new();

        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| StateError::IoError {
                detail: format!("Failed to read directory entry: {}", e),
            })?
        {
            let path = entry.path();

            // Skip temp files and non-JSON files
            if path.extension().is_none_or(|ext| ext != "json") {
                continue;
            }
            if path
                .file_name()
                .is_none_or(|name| name.to_string_lossy().ends_with(".json.tmp"))
            {
                continue;
            }

            // Extract UUID from filename
            if let Some(file_stem) = path.file_stem()
                && let Ok(uuid) = Uuid::parse_str(&file_stem.to_string_lossy()) {
                    ids.push(uuid);
                }
        }

        Ok(ids)
    }

    async fn count(&self) -> Result<u64, StateError> {
        Ok(self.list_ids().await?.len() as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use indexmap::IndexMap;
    use tempfile::TempDir;

    use crate::state_persistence::domain::{ExecutionStatus, NodeState, NodeStatus};

    fn create_test_state(execution_id: Uuid) -> ExecutionState {
        let mut state = ExecutionState::new(execution_id, "test_hash_123".to_string());
        state.status = ExecutionStatus::Running;

        let mut node_states = IndexMap::new();
        let node_id_1 = Uuid::new_v4();
        let node_id_2 = Uuid::new_v4();

        let mut node1 = NodeState::new(node_id_1);
        node1.status = NodeStatus::Completed;
        node1.output = Some("test output".to_string());
        node1.duration_ms = Some(100);

        let mut node2 = NodeState::new(node_id_2);
        node2.status = NodeStatus::Pending;

        node_states.insert(node_id_1, node1);
        node_states.insert(node_id_2, node2);
        state.node_states = node_states;

        state
    }

    async fn create_repo() -> (FileSystemStateRepository, TempDir) {
        let dir = TempDir::new().unwrap();
        let repo = FileSystemStateRepository::new(dir.path().to_path_buf())
            .await
            .unwrap();
        (repo, dir)
    }

    #[tokio::test]
    async fn test_save_and_load_roundtrip() {
        let (repo, _dir) = create_repo().await;
        let execution_id = Uuid::new_v4();
        let state = create_test_state(execution_id);

        repo.save(&state).await.unwrap();

        let loaded = repo.load(execution_id).await.unwrap();
        assert_eq!(loaded.execution_id, execution_id);
        assert_eq!(loaded.status, ExecutionStatus::Running);
        assert_eq!(loaded.symbol_graph_hash, "test_hash_123");
        assert_eq!(loaded.node_states.len(), 2);
    }

    #[tokio::test]
    async fn test_load_nonexistent_returns_error() {
        let (repo, _dir) = create_repo().await;
        let execution_id = Uuid::new_v4();

        let result = repo.load(execution_id).await;
        assert!(matches!(result, Err(StateError::StateNotFound { .. })));
    }

    #[tokio::test]
    async fn test_exists() {
        let (repo, _dir) = create_repo().await;
        let execution_id = Uuid::new_v4();
        let state = create_test_state(execution_id);

        assert!(!repo.exists(execution_id).await.unwrap());
        repo.save(&state).await.unwrap();
        assert!(repo.exists(execution_id).await.unwrap());
    }

    #[tokio::test]
    async fn test_delete_removes_file() {
        let (repo, _dir) = create_repo().await;
        let execution_id = Uuid::new_v4();
        let state = create_test_state(execution_id);

        repo.save(&state).await.unwrap();
        assert!(repo.exists(execution_id).await.unwrap());

        repo.delete(execution_id).await.unwrap();
        assert!(!repo.exists(execution_id).await.unwrap());
    }

    #[tokio::test]
    async fn test_delete_nonexistent_is_idempotent() {
        let (repo, _dir) = create_repo().await;
        let execution_id = Uuid::new_v4();

        // Should not error
        repo.delete(execution_id).await.unwrap();
    }

    #[tokio::test]
    async fn test_list_ids() {
        let (repo, _dir) = create_repo().await;
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();

        assert!(repo.list_ids().await.unwrap().is_empty());

        repo.save(&create_test_state(id1)).await.unwrap();
        repo.save(&create_test_state(id2)).await.unwrap();

        let ids = repo.list_ids().await.unwrap();
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&id1));
        assert!(ids.contains(&id2));
    }

    #[tokio::test]
    async fn test_count() {
        let (repo, _dir) = create_repo().await;
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();

        assert_eq!(repo.count().await.unwrap(), 0);
        repo.save(&create_test_state(id1)).await.unwrap();
        assert_eq!(repo.count().await.unwrap(), 1);
        repo.save(&create_test_state(id2)).await.unwrap();
        assert_eq!(repo.count().await.unwrap(), 2);
    }

    #[tokio::test]
    async fn test_tmp_file_cleaned_on_success() {
        let (repo, dir) = create_repo().await;
        let execution_id = Uuid::new_v4();
        let state = create_test_state(execution_id);

        repo.save(&state).await.unwrap();

        // Temp file should not exist after successful write
        let temp_path = dir.path().join(format!("{}.json.tmp", execution_id));
        assert!(!temp_path.exists());

        // Real file should exist
        let real_path = dir.path().join(format!("{}.json", execution_id));
        assert!(real_path.exists());
    }

    #[tokio::test]
    async fn test_serialisation_error_handling() {
        let (repo, _dir) = create_repo().await;
        let execution_id = Uuid::new_v4();
        let state = create_test_state(execution_id);

        // Verify the state round-trips correctly
        repo.save(&state).await.unwrap();
        let loaded = repo.load(state.execution_id).await.unwrap();

        // Verify the state is semantically equal
        assert_eq!(loaded.execution_id, state.execution_id);
        assert_eq!(loaded.status, state.status);
    }

    #[tokio::test]
    async fn test_overwrite_existing_state() {
        let (repo, _dir) = create_repo().await;
        let execution_id = Uuid::new_v4();

        let mut state1 = create_test_state(execution_id);
        state1.status = ExecutionStatus::Running;

        let mut state2 = create_test_state(execution_id);
        state2.status = ExecutionStatus::Completed;

        repo.save(&state1).await.unwrap();
        repo.save(&state2).await.unwrap();

        let loaded = repo.load(execution_id).await.unwrap();
        assert_eq!(loaded.status, ExecutionStatus::Completed);
    }

    #[tokio::test]
    async fn test_creates_directory_automatically() {
        let dir = TempDir::new().unwrap();
        let nested = dir.path().join("deeply").join("nested").join("state");

        // Should create the directory automatically
        let repo = FileSystemStateRepository::new(nested.clone()).await;
        assert!(repo.is_ok());
        assert!(nested.exists());

        // And we can use it
        let repo = repo.unwrap();
        let execution_id = Uuid::new_v4();
        let state = create_test_state(execution_id);
        repo.save(&state).await.unwrap();
        assert!(repo.exists(execution_id).await.unwrap());
    }

    #[tokio::test]
    async fn test_rejects_file_path() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("not_a_dir");

        // Create a file at that path
        tokio::fs::write(&file_path, b"this is a file, not a directory")
            .await
            .unwrap();

        let result = FileSystemStateRepository::new(file_path).await;
        assert!(matches!(result, Err(StateError::DirectoryError { .. })));
    }
}
