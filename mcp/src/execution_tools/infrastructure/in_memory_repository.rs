//! In-memory implementation of ExecutionRepository.
//!
//! @canonical .pi/architecture/modules/execution-tools.md#repositories
//! Implements: ExecutionRepository — in-memory storage for execution results

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::RwLock;

use crate::execution_tools::domain::error::EngineFacadeError;
use crate::execution_tools::domain::value::{CostBreakdown, ExecutionId, ExecutionResult};

use super::repository::ExecutionRepository;

/// Thread-safe in-memory execution repository.
pub struct InMemoryExecutionRepository {
    executions: RwLock<HashMap<ExecutionId, ExecutionResult>>,
}

impl InMemoryExecutionRepository {
    /// Create a new empty repository.
    pub fn new() -> Self {
        Self {
            executions: RwLock::new(HashMap::new()),
        }
    }
}

impl Default for InMemoryExecutionRepository {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ExecutionRepository for InMemoryExecutionRepository {
    async fn find_execution(
        &self,
        execution_id: &ExecutionId,
    ) -> Result<Option<ExecutionResult>, EngineFacadeError> {
        let map = self
            .executions
            .read()
            .map_err(|e| EngineFacadeError::Internal(format!("Lock poisoned: {}", e)))?;
        Ok(map.get(execution_id).cloned())
    }

    async fn save_execution(&self, execution: &ExecutionResult) -> Result<(), EngineFacadeError> {
        let id = *execution.execution_id();
        let exec_id = ExecutionId::from_uuid(id);
        let mut map = self
            .executions
            .write()
            .map_err(|e| EngineFacadeError::Internal(format!("Lock poisoned: {}", e)))?;
        map.insert(exec_id, execution.clone());
        Ok(())
    }

    async fn find_cost_breakdown(
        &self,
        execution_id: &ExecutionId,
    ) -> Result<Option<CostBreakdown>, EngineFacadeError> {
        let map = self
            .executions
            .read()
            .map_err(|e| EngineFacadeError::Internal(format!("Lock poisoned: {}", e)))?;
        Ok(map.get(execution_id).map(|exec| {
            let step_count = exec.steps().len();
            let tool_calls = exec.steps().iter().filter(|s| s.is_success()).count() as u64;
            CostBreakdown::new(
                *exec.execution_id(),
                exec.steps()
                    .iter()
                    .map(|s| crate::execution_tools::domain::value::StepCost {
                        step_name: s.step_name().to_string(),
                        tokens: 0,
                        tool_calls: if s.is_success() { 1 } else { 0 },
                        cost_micro: None,
                    })
                    .collect(),
                exec.tokens_used().unwrap_or(0),
                tool_calls.max(step_count as u64),
                None,
            )
        }))
    }
}
