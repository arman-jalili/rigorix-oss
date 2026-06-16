//! Implementation of the StateManagerFactory.
//!
//! @canonical .pi/architecture/modules/state-persistence.md#application
//! Implements: ISSUE-STATE-PERSISTENCE-1 — FileSystemStateManagerFactory
//! Issue: #79
//!
//! Provides the concrete `FileSystemStateManagerFactory` that constructs
//! `FileSystemStateManager` instances backed by `FileSystemStateRepository`.

use async_trait::async_trait;
use std::path::PathBuf;

use crate::state_persistence::application::factory::{
    CreateStateManagerConfig, StateManagerFactory,
};
use crate::state_persistence::application::service::StateManagerService;
use crate::state_persistence::application::state_manager_service_impl::FileSystemStateManager;
use crate::state_persistence::domain::StateError;
use crate::state_persistence::infrastructure::FileSystemStateRepository;

/// Factory for constructing `FileSystemStateManager` instances.
///
/// Creates state managers backed by the local filesystem using atomic
/// write-rename for crash safety.
pub struct FileSystemStateManagerFactory;

#[async_trait]
impl StateManagerFactory for FileSystemStateManagerFactory {
    async fn create(
        &self,
        state_dir: PathBuf,
        _config: CreateStateManagerConfig,
    ) -> Result<Box<dyn StateManagerService>, StateError> {
        // TODO: config.create_dir_if_missing handling (currently both branches identical)
        let repo = FileSystemStateRepository::new(state_dir).await?;

        let manager = FileSystemStateManager::new(Box::new(repo));
        Ok(Box::new(manager))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    use crate::state_persistence::application::dto::SaveStateInput;
    use crate::state_persistence::domain::ExecutionState;
    use crate::state_persistence::domain::ExecutionStatus;
    use uuid::Uuid;

    #[tokio::test]
    async fn test_factory_create_default() {
        let dir = TempDir::new().unwrap();
        let factory = FileSystemStateManagerFactory;

        let manager = factory
            .create(
                dir.path().to_path_buf(),
                CreateStateManagerConfig::default(),
            )
            .await
            .unwrap();

        // Verify we can use the manager
        let execution_id = Uuid::new_v4();
        let state = ExecutionState::new(execution_id, "hash".to_string());
        let output = manager.save_state(SaveStateInput { state }).await.unwrap();
        assert_eq!(output.status, ExecutionStatus::Pending);
    }

    #[tokio::test]
    async fn test_factory_creates_directory() {
        let dir = TempDir::new().unwrap();
        let nested_path = dir.path().join("nested").join("state").join("dir");
        let factory = FileSystemStateManagerFactory;

        let result = factory
            .create(
                nested_path.clone(),
                CreateStateManagerConfig {
                    create_dir_if_missing: true,
                    ..Default::default()
                },
            )
            .await;

        assert!(result.is_ok());
        assert!(nested_path.exists());
    }
}
