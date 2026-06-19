//! In-memory LlmGenerateNode repository implementation.
//!
//! @canonical .pi/architecture/modules/llm-step.md
//! Implements: LlmGenerateNode — in-memory node repository
//! Issue: issue-llmgeneratenode
//!
//! Provides an in-memory HashMap-backed implementation of
//! LlmGenerateNodeRepository for testing and single-process use.

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Mutex;
use uuid::Uuid;

use crate::llm_step::domain::{LlmGenerateNode, LlmStepError};

use super::LlmGenerateNodeRepository;

/// In-memory implementation of LlmGenerateNodeRepository.
///
/// Stores nodes in a HashMap keyed by UUID. Not suitable for
/// production multi-process use but provides the contract-level
/// behavior needed for testing and single-process execution.
pub struct InMemoryNodeRepository {
    /// In-memory node store.
    nodes: Mutex<HashMap<Uuid, LlmGenerateNode>>,
}

impl InMemoryNodeRepository {
    /// Create a new empty InMemoryNodeRepository.
    pub fn new() -> Self {
        Self {
            nodes: Mutex::new(HashMap::new()),
        }
    }
}

impl Default for InMemoryNodeRepository {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl LlmGenerateNodeRepository for InMemoryNodeRepository {
    async fn save(&self, node: &LlmGenerateNode) -> Result<(), LlmStepError> {
        let mut nodes = self.nodes.lock().map_err(|e| {
            LlmStepError::MissingDependency {
                dependency: "InMemoryNodeRepository lock".to_string(),
                resolution: Some("This is a fatal internal error".to_string()),
            }
        })?;
        nodes.insert(node.id, node.clone());
        Ok(())
    }

    async fn load(&self, node_id: Uuid) -> Result<LlmGenerateNode, LlmStepError> {
        let nodes = self.nodes.lock().map_err(|e| {
            LlmStepError::MissingDependency {
                dependency: "InMemoryNodeRepository lock".to_string(),
                resolution: Some("This is a fatal internal error".to_string()),
            }
        })?;
        nodes.get(&node_id).cloned().ok_or_else(|| {
            LlmStepError::MissingDependency {
                dependency: format!("Node {}", node_id),
                resolution: Some("Check that the node was created before loading".to_string()),
            }
        })
    }

    async fn exists(&self, node_id: Uuid) -> Result<bool, LlmStepError> {
        let nodes = self.nodes.lock().map_err(|e| {
            LlmStepError::MissingDependency {
                dependency: "InMemoryNodeRepository lock".to_string(),
                resolution: Some("This is a fatal internal error".to_string()),
            }
        })?;
        Ok(nodes.contains_key(&node_id))
    }

    async fn delete(&self, node_id: Uuid) -> Result<(), LlmStepError> {
        let mut nodes = self.nodes.lock().map_err(|e| {
            LlmStepError::MissingDependency {
                dependency: "InMemoryNodeRepository lock".to_string(),
                resolution: Some("This is a fatal internal error".to_string()),
            }
        })?;
        nodes.remove(&node_id);
        Ok(())
    }

    async fn list_ids(&self) -> Result<Vec<Uuid>, LlmStepError> {
        let nodes = self.nodes.lock().map_err(|e| {
            LlmStepError::MissingDependency {
                dependency: "InMemoryNodeRepository lock".to_string(),
                resolution: Some("This is a fatal internal error".to_string()),
            }
        })?;
        Ok(nodes.keys().copied().collect())
    }

    async fn count(&self) -> Result<u64, LlmStepError> {
        let nodes = self.nodes.lock().map_err(|e| {
            LlmStepError::MissingDependency {
                dependency: "InMemoryNodeRepository lock".to_string(),
                resolution: Some("This is a fatal internal error".to_string()),
            }
        })?;
        Ok(nodes.len() as u64)
    }

    async fn find_by_execution(
        &self,
        _execution_id: Uuid,
    ) -> Result<Vec<LlmGenerateNode>, LlmStepError> {
        // In-memory implementation doesn't track execution associations
        // Return all nodes. A production implementation would index by execution_id.
        let nodes = self.nodes.lock().map_err(|e| {
            LlmStepError::MissingDependency {
                dependency: "InMemoryNodeRepository lock".to_string(),
                resolution: Some("This is a fatal internal error".to_string()),
            }
        })?;
        Ok(nodes.values().cloned().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm_step::domain::{
        LlmGenerateNode, LlmGenerateNodeState, LlmModelConfig, LlmOutputFormat, LlmOutputSchema,
    };
    use chrono::Utc;

    fn create_test_node(id: Uuid) -> LlmGenerateNode {
        LlmGenerateNode {
            id,
            name: format!("test-node-{}", id),
            model_config: LlmModelConfig::default(),
            prompt_template: "Generate code for {{source_code}}".to_string(),
            output_schema: LlmOutputSchema {
                format: LlmOutputFormat::Text,
                schema: "plain text".to_string(),
                strict: false,
            },
            state: LlmGenerateNodeState::Created,
            output: None,
            error: None,
            created_at: Utc::now(),
            started_at: None,
            completed_at: None,
        }
    }

    #[tokio::test]
    async fn test_save_and_load() {
        let repo = InMemoryNodeRepository::new();
        let node_id = Uuid::new_v4();
        let node = create_test_node(node_id);

        repo.save(&node).await.unwrap();
        let loaded = repo.load(node_id).await.unwrap();

        assert_eq!(loaded.id, node_id);
        assert_eq!(loaded.name, node.name);
    }

    #[tokio::test]
    async fn test_load_nonexistent() {
        let repo = InMemoryNodeRepository::new();
        let node_id = Uuid::new_v4();

        let result = repo.load(node_id).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_exists() {
        let repo = InMemoryNodeRepository::new();
        let node_id = Uuid::new_v4();
        let node = create_test_node(node_id);

        assert!(!repo.exists(node_id).await.unwrap());
        repo.save(&node).await.unwrap();
        assert!(repo.exists(node_id).await.unwrap());
    }

    #[tokio::test]
    async fn test_delete() {
        let repo = InMemoryNodeRepository::new();
        let node_id = Uuid::new_v4();
        let node = create_test_node(node_id);

        repo.save(&node).await.unwrap();
        assert!(repo.exists(node_id).await.unwrap());

        repo.delete(node_id).await.unwrap();
        assert!(!repo.exists(node_id).await.unwrap());
    }

    #[tokio::test]
    async fn test_delete_idempotent() {
        let repo = InMemoryNodeRepository::new();
        let node_id = Uuid::new_v4();

        // Delete non-existent node should not error
        let result = repo.delete(node_id).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_count() {
        let repo = InMemoryNodeRepository::new();
        assert_eq!(repo.count().await.unwrap(), 0);

        let node1 = create_test_node(Uuid::new_v4());
        let node2 = create_test_node(Uuid::new_v4());

        repo.save(&node1).await.unwrap();
        assert_eq!(repo.count().await.unwrap(), 1);

        repo.save(&node2).await.unwrap();
        assert_eq!(repo.count().await.unwrap(), 2);
    }

    #[tokio::test]
    async fn test_list_ids() {
        let repo = InMemoryNodeRepository::new();
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();

        repo.save(&create_test_node(id1)).await.unwrap();
        repo.save(&create_test_node(id2)).await.unwrap();

        let ids = repo.list_ids().await.unwrap();
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&id1));
        assert!(ids.contains(&id2));
    }
}
