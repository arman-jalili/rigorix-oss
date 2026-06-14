//! Implementation of the GraphManagerFactory.
//!
//! @canonical .pi/architecture/modules/state-persistence.md#application
//! Implements: ISSUE-STATE-PERSISTENCE-2 — FileSystemGraphManagerFactory
//! Issue: #80
//!
//! Provides the concrete `FileSystemGraphManagerFactory` that constructs
//! `FileSystemGraphManager` instances backed by `FileSystemGraphRepository`.

use async_trait::async_trait;
use std::path::PathBuf;

use crate::state_persistence::application::factory::{CreateGraphManagerConfig, GraphManagerFactory};
use crate::state_persistence::application::graph_manager_service_impl::FileSystemGraphManager;
use crate::state_persistence::application::service::GraphManagerService;
use crate::state_persistence::domain::StateError;
use crate::state_persistence::infrastructure::FileSystemGraphRepository;

/// Factory for constructing `FileSystemGraphManager` instances.
pub struct FileSystemGraphManagerFactory;

#[async_trait]
impl GraphManagerFactory for FileSystemGraphManagerFactory {
    async fn create(
        &self,
        graph_dir: PathBuf,
        _config: CreateGraphManagerConfig,
    ) -> Result<Box<dyn GraphManagerService>, StateError> {
        let repo = FileSystemGraphRepository::new(graph_dir).await?;
        let manager = FileSystemGraphManager::new(Box::new(repo));
        Ok(Box::new(manager))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use uuid::Uuid;

    use crate::state_persistence::application::dto::ExecutionSummary;
    use crate::state_persistence::domain::ExecutionStatus;
    use chrono::Utc;

    #[tokio::test]
    async fn test_factory_create_default() {
        let dir = TempDir::new().unwrap();
        let factory = FileSystemGraphManagerFactory;

        let manager = factory
            .create(dir.path().to_path_buf(), CreateGraphManagerConfig::default())
            .await
            .unwrap();

        // Should be usable
        let summary = ExecutionSummary {
            execution_id: Uuid::new_v4(),
            status: ExecutionStatus::Completed,
            started_at: Utc::now(),
            completed_at: Some(Utc::now()),
            duration_ms: Some(100),
            node_count: 1,
            completed_node_count: 1,
            failed_node_count: 0,
            skipped_node_count: 0,
            has_active_nodes: false,
        };
        manager.save_graph(&summary).await.unwrap();

        let list = manager.list_graphs(10).await.unwrap();
        assert_eq!(list.executions.len(), 1);
    }
}
