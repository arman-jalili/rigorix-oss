//! In-memory repository implementation for CodeGraph persistence.
//!
//! @canonical .pi/architecture/modules/code-graph.md#infrastructure
//! Implements: PersistedCodeGraph — InMemoryCodeGraphRepository
//! Issue: issue-persistedcodegraph
//!
//! Provides a thread-safe in-memory implementation of CodeGraphRepository
//! suitable for testing and single-process use.

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Mutex;
use uuid::Uuid;

use crate::code_graph::domain::{CodeGraph, CodeGraphError};

use super::CodeGraphRepository;

/// Thread-safe in-memory implementation of CodeGraphRepository.
///
/// Stores CodeGraph instances in a HashMap. All operations are O(1) except
/// `search` and `list_ids` which are O(n).
///
/// Not suitable for production multi-process use. For production, use
/// a filesystem or database-backed repository implementation.
pub struct InMemoryCodeGraphRepository {
    /// Internal graph storage.
    store: Mutex<HashMap<Uuid, CodeGraph>>,
}

impl InMemoryCodeGraphRepository {
    /// Create a new empty in-memory repository.
    pub fn new() -> Self {
        Self {
            store: Mutex::new(HashMap::new()),
        }
    }

    /// Create a new in-memory repository pre-populated with graphs.
    pub fn with_graphs(graphs: HashMap<Uuid, CodeGraph>) -> Self {
        Self {
            store: Mutex::new(graphs),
        }
    }
}

impl Default for InMemoryCodeGraphRepository {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl CodeGraphRepository for InMemoryCodeGraphRepository {
    async fn save(&self, graph: &CodeGraph) -> Result<(), CodeGraphError> {
        let mut store = self
            .store
            .lock()
            .map_err(|e| CodeGraphError::InternalError {
                detail: format!("Lock error: {}", e),
            })?;

        // Find existing entry by comparing metadata timestamps, or generate new ID
        let id = Uuid::new_v4();
        store.insert(id, graph.clone());
        Ok(())
    }

    async fn load(&self, graph_id: Uuid) -> Result<CodeGraph, CodeGraphError> {
        let store = self
            .store
            .lock()
            .map_err(|e| CodeGraphError::InternalError {
                detail: format!("Lock error: {}", e),
            })?;

        store
            .get(&graph_id)
            .cloned()
            .ok_or_else(|| CodeGraphError::InvalidOperation {
                reason: format!("Graph not found: {}", graph_id),
            })
    }

    async fn exists(&self, graph_id: Uuid) -> Result<bool, CodeGraphError> {
        let store = self
            .store
            .lock()
            .map_err(|e| CodeGraphError::InternalError {
                detail: format!("Lock error: {}", e),
            })?;
        Ok(store.contains_key(&graph_id))
    }

    async fn delete(&self, graph_id: Uuid) -> Result<(), CodeGraphError> {
        let mut store = self
            .store
            .lock()
            .map_err(|e| CodeGraphError::InternalError {
                detail: format!("Lock error: {}", e),
            })?;
        store.remove(&graph_id);
        Ok(())
    }

    async fn list_ids(&self) -> Result<Vec<Uuid>, CodeGraphError> {
        let store = self
            .store
            .lock()
            .map_err(|e| CodeGraphError::InternalError {
                detail: format!("Lock error: {}", e),
            })?;
        Ok(store.keys().copied().collect())
    }

    async fn list_ids_paginated(
        &self,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<Uuid>, CodeGraphError> {
        let store = self
            .store
            .lock()
            .map_err(|e| CodeGraphError::InternalError {
                detail: format!("Lock error: {}", e),
            })?;

        let mut ids: Vec<Uuid> = store.keys().copied().collect();
        ids.sort();
        Ok(ids
            .into_iter()
            .skip(offset as usize)
            .take(limit as usize)
            .collect())
    }

    async fn count(&self) -> Result<u64, CodeGraphError> {
        let store = self
            .store
            .lock()
            .map_err(|e| CodeGraphError::InternalError {
                detail: format!("Lock error: {}", e),
            })?;
        Ok(store.len() as u64)
    }

    async fn search(&self, query: &str, limit: u32) -> Result<Vec<CodeGraph>, CodeGraphError> {
        let store = self
            .store
            .lock()
            .map_err(|e| CodeGraphError::InternalError {
                detail: format!("Lock error: {}", e),
            })?;

        let query_lower = query.to_lowercase();
        let mut results: Vec<CodeGraph> = store
            .values()
            .filter(|g| {
                g.metadata.name.to_lowercase().contains(&query_lower)
                    || g.metadata.source.to_lowercase().contains(&query_lower)
                    || g.metadata.description.to_lowercase().contains(&query_lower)
            })
            .take(limit as usize)
            .cloned()
            .collect();

        results.sort_by_key(|b| std::cmp::Reverse(b.metadata.created_at));
        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::code_graph::domain::GraphMetadata;
    use chrono::Utc;

    fn create_test_graph(name: &str) -> CodeGraph {
        CodeGraph::new(GraphMetadata {
            name: name.to_string(),
            source: "test".to_string(),
            created_at: Utc::now(),
            description: "Test graph".to_string(),
            total_modules_scanned: 0,
            schema_version: "1.0.0".to_string(),
        })
    }

    #[tokio::test]
    async fn test_save_and_load() {
        let repo = InMemoryCodeGraphRepository::new();
        let graph = create_test_graph("test-graph");
        repo.save(&graph).await.unwrap();

        let all_ids = repo.list_ids().await.unwrap();
        assert_eq!(all_ids.len(), 1);

        let loaded = repo.load(all_ids[0]).await.unwrap();
        assert_eq!(loaded.metadata.name, "test-graph");
    }

    #[tokio::test]
    async fn test_exists() {
        let repo = InMemoryCodeGraphRepository::new();
        let graph = create_test_graph("test");
        repo.save(&graph).await.unwrap();

        let all_ids = repo.list_ids().await.unwrap();
        assert!(repo.exists(all_ids[0]).await.unwrap());
        assert!(!repo.exists(Uuid::new_v4()).await.unwrap());
    }

    #[tokio::test]
    async fn test_delete() {
        let repo = InMemoryCodeGraphRepository::new();
        let graph = create_test_graph("test");
        repo.save(&graph).await.unwrap();

        let all_ids = repo.list_ids().await.unwrap();
        assert_eq!(all_ids.len(), 1);

        repo.delete(all_ids[0]).await.unwrap();
        assert_eq!(repo.list_ids().await.unwrap().len(), 0);
    }

    #[tokio::test]
    async fn test_count() {
        let repo = InMemoryCodeGraphRepository::new();
        assert_eq!(repo.count().await.unwrap(), 0);

        repo.save(&create_test_graph("a")).await.unwrap();
        repo.save(&create_test_graph("b")).await.unwrap();
        assert_eq!(repo.count().await.unwrap(), 2);
    }

    #[tokio::test]
    async fn test_search() {
        let repo = InMemoryCodeGraphRepository::new();
        repo.save(&create_test_graph("cargo-deps")).await.unwrap();
        repo.save(&create_test_graph("ts-metrics")).await.unwrap();

        let results = repo.search("cargo", 10).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].metadata.name, "cargo-deps");
    }

    #[tokio::test]
    async fn test_pagination() {
        let repo = InMemoryCodeGraphRepository::new();
        for i in 0..10 {
            repo.save(&create_test_graph(&format!("graph-{}", i)))
                .await
                .unwrap();
        }

        let page1 = repo.list_ids_paginated(3, 0).await.unwrap();
        assert_eq!(page1.len(), 3);

        let page2 = repo.list_ids_paginated(3, 3).await.unwrap();
        assert_eq!(page2.len(), 3);
    }

    #[tokio::test]
    async fn test_load_nonexistent() {
        let repo = InMemoryCodeGraphRepository::new();
        let result = repo.load(Uuid::new_v4()).await;
        assert!(result.is_err());
    }
}
