//! In-memory `ExecutionResultRepository` + `RetryDecisionRepository`.
//!
//! @canonical .pi/architecture/modules/execution-engine.md
//! Implements: GAP-A-16 — execution repository impls
//!
//! Stores execution results, in-flight node states, and retry decisions
//! in memory keyed by dag_id.

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Mutex;
use uuid::Uuid;

use crate::execution_engine::domain::error::ExecutionError;
use crate::execution_engine::domain::parallel_executor::{ExecutionResult, NodeExecutionState};
use crate::execution_engine::domain::retry::RetryDecision;
use crate::execution_engine::infrastructure::repository::{
    ExecutionResultRepository, RetryDecisionRepository,
};

fn internal(detail: impl Into<String>) -> ExecutionError {
    ExecutionError::InternalError {
        detail: detail.into(),
    }
}

/// In-memory execution result + in-flight state store.
pub struct InMemoryExecutionResultRepository {
    results: Mutex<HashMap<Uuid, ExecutionResult>>,
    states: Mutex<HashMap<Uuid, Vec<NodeExecutionState>>>,
}

impl InMemoryExecutionResultRepository {
    /// Create an empty repository.
    pub fn new() -> Self {
        Self {
            results: Mutex::new(HashMap::new()),
            states: Mutex::new(HashMap::new()),
        }
    }
}

impl Default for InMemoryExecutionResultRepository {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ExecutionResultRepository for InMemoryExecutionResultRepository {
    async fn save_result(&self, result: &ExecutionResult) -> Result<(), ExecutionError> {
        self.results
            .lock()
            .map_err(|_| internal("results poisoned"))?
            .insert(result.dag_id, result.clone());
        Ok(())
    }

    async fn load_result(&self, dag_id: Uuid) -> Result<ExecutionResult, ExecutionError> {
        self.results
            .lock()
            .map_err(|_| internal("results poisoned"))?
            .get(&dag_id)
            .cloned()
            .ok_or(ExecutionError::NodeNotFound { node_id: dag_id })
    }

    async fn exists(&self, dag_id: Uuid) -> Result<bool, ExecutionError> {
        Ok(self
            .results
            .lock()
            .map_err(|_| internal("results poisoned"))?
            .contains_key(&dag_id))
    }

    async fn save_state(
        &self,
        dag_id: Uuid,
        node_states: &[NodeExecutionState],
    ) -> Result<(), ExecutionError> {
        self.states
            .lock()
            .map_err(|_| internal("states poisoned"))?
            .insert(dag_id, node_states.to_vec());
        Ok(())
    }

    async fn load_state(&self, dag_id: Uuid) -> Result<Vec<NodeExecutionState>, ExecutionError> {
        Ok(self
            .states
            .lock()
            .map_err(|_| internal("states poisoned"))?
            .get(&dag_id)
            .cloned()
            .unwrap_or_default())
    }

    async fn delete_execution(&self, dag_id: Uuid) -> Result<(), ExecutionError> {
        self.results
            .lock()
            .map_err(|_| internal("results poisoned"))?
            .remove(&dag_id);
        self.states
            .lock()
            .map_err(|_| internal("states poisoned"))?
            .remove(&dag_id);
        Ok(())
    }

    async fn list_executions(&self) -> Result<Vec<Uuid>, ExecutionError> {
        let mut ids: Vec<Uuid> = self
            .results
            .lock()
            .map_err(|_| internal("results poisoned"))?
            .keys()
            .cloned()
            .collect();
        ids.sort();
        Ok(ids)
    }

    async fn count(&self) -> Result<u64, ExecutionError> {
        Ok(self
            .results
            .lock()
            .map_err(|_| internal("results poisoned"))?
            .len() as u64)
    }
}

/// In-memory retry decision audit log.
pub struct InMemoryRetryDecisionRepository {
    decisions: Mutex<HashMap<Uuid, Vec<(Uuid, RetryDecision)>>>,
}

impl InMemoryRetryDecisionRepository {
    /// Create an empty audit log.
    pub fn new() -> Self {
        Self {
            decisions: Mutex::new(HashMap::new()),
        }
    }
}

impl Default for InMemoryRetryDecisionRepository {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl RetryDecisionRepository for InMemoryRetryDecisionRepository {
    async fn save_decision(
        &self,
        dag_id: Uuid,
        node_id: Uuid,
        decision: &RetryDecision,
    ) -> Result<(), ExecutionError> {
        self.decisions
            .lock()
            .map_err(|_| internal("decisions poisoned"))?
            .entry(dag_id)
            .or_default()
            .push((node_id, decision.clone()));
        Ok(())
    }

    async fn load_decisions(
        &self,
        dag_id: Uuid,
    ) -> Result<Vec<(Uuid, RetryDecision)>, ExecutionError> {
        Ok(self
            .decisions
            .lock()
            .map_err(|_| internal("decisions poisoned"))?
            .get(&dag_id)
            .cloned()
            .unwrap_or_default())
    }

    async fn delete_decisions(&self, dag_id: Uuid) -> Result<(), ExecutionError> {
        self.decisions
            .lock()
            .map_err(|_| internal("decisions poisoned"))?
            .remove(&dag_id);
        Ok(())
    }

    async fn prune(&self, max_records: u64) -> Result<u64, ExecutionError> {
        let mut decisions = self
            .decisions
            .lock()
            .map_err(|_| internal("decisions poisoned"))?;
        let mut removed = 0u64;
        for entry in decisions.values_mut() {
            while entry.len() as u64 > max_records {
                entry.remove(0);
                removed += 1;
            }
        }
        Ok(removed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::execution_engine::domain::parallel_executor::ExecutionResult;

    #[tokio::test]
    async fn test_execution_result_round_trip() {
        let repo = InMemoryExecutionResultRepository::new();
        let dag_id = Uuid::new_v4();
        let result = ExecutionResult {
            dag_id,
            node_results: HashMap::new(),
            execution_states: HashMap::new(),
            completed_count: 1,
            failed_count: 0,
            skipped_count: 0,
            total_nodes: 1,
            total_duration_ms: 5,
            total_retries: 0,
            started_at: chrono::Utc::now(),
            completed_at: chrono::Utc::now(),
            cancelled: false,
            cancellation_reason: None,
        };
        repo.save_result(&result).await.unwrap();
        assert!(repo.exists(dag_id).await.unwrap());
        assert_eq!(repo.count().await.unwrap(), 1);
        let loaded = repo.load_result(dag_id).await.unwrap();
        assert_eq!(loaded.dag_id, dag_id);
        assert_eq!(repo.list_executions().await.unwrap(), vec![dag_id]);
        repo.delete_execution(dag_id).await.unwrap();
        assert!(!repo.exists(dag_id).await.unwrap());
    }

    #[tokio::test]
    async fn test_retry_decision_round_trip() {
        let repo = InMemoryRetryDecisionRepository::new();
        let dag_id = Uuid::new_v4();
        let node_id = Uuid::new_v4();
        repo.save_decision(
            dag_id,
            node_id,
            &RetryDecision::Retry {
                strategy: crate::execution_engine::domain::retry::RetryStrategy::SameOperation,
                attempt: 1,
                backoff_ms: 100,
                reason: "test".to_string(),
            },
        )
        .await
        .unwrap();
        let decisions = repo.load_decisions(dag_id).await.unwrap();
        assert_eq!(decisions.len(), 1);
        repo.prune(0).await.unwrap();
        assert!(repo.load_decisions(dag_id).await.unwrap().is_empty());
    }
}
