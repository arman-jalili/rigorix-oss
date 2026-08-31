//! In-memory `LlmBudgetRepository` implementation.
//!
//! @canonical .pi/architecture/modules/budget-tracking.md
//! Implements: GAP-A-16 — LlmBudgetRepository impl
//!
//! Stores budget snapshots (by label) and reservation records (by
//! execution_id) in memory for audit/replay.

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Mutex;
use uuid::Uuid;

use crate::budget_tracking::application::dto::{CommitReservationInput, ReserveBudgetInput};
use crate::budget_tracking::domain::budget::LlmBudget;
use crate::budget_tracking::domain::error::LlmBudgetError;
use crate::budget_tracking::infrastructure::repository::LlmBudgetRepository;

/// In-memory budget repository.
pub struct InMemoryLlmBudgetRepository {
    budgets: Mutex<HashMap<String, LlmBudget>>,
    reservations: Mutex<HashMap<Uuid, ReserveBudgetInput>>,
    commits: Mutex<HashMap<Uuid, CommitReservationInput>>,
}

impl InMemoryLlmBudgetRepository {
    /// Create an empty repository.
    pub fn new() -> Self {
        Self {
            budgets: Mutex::new(HashMap::new()),
            reservations: Mutex::new(HashMap::new()),
            commits: Mutex::new(HashMap::new()),
        }
    }
}

impl Default for InMemoryLlmBudgetRepository {
    fn default() -> Self {
        Self::new()
    }
}

fn internal(detail: impl Into<String>) -> LlmBudgetError {
    LlmBudgetError::Internal {
        detail: detail.into(),
    }
}

#[async_trait]
impl LlmBudgetRepository for InMemoryLlmBudgetRepository {
    async fn save(&self, budget: &LlmBudget) -> Result<(), LlmBudgetError> {
        self.budgets
            .lock()
            .map_err(|_| internal("budget store poisoned"))?
            .insert(budget.label.clone(), budget.clone());
        Ok(())
    }

    async fn find_by_label(&self, label: &str) -> Result<Option<LlmBudget>, LlmBudgetError> {
        Ok(self
            .budgets
            .lock()
            .map_err(|_| internal("budget store poisoned"))?
            .get(label)
            .cloned())
    }

    async fn record_reservation(
        &self,
        execution_id: &Uuid,
        input: &ReserveBudgetInput,
    ) -> Result<(), LlmBudgetError> {
        self.reservations
            .lock()
            .map_err(|_| internal("reservation store poisoned"))?
            .insert(*execution_id, input.clone());
        Ok(())
    }

    async fn record_commit(
        &self,
        execution_id: &Uuid,
        input: &CommitReservationInput,
    ) -> Result<(), LlmBudgetError> {
        self.commits
            .lock()
            .map_err(|_| internal("commit store poisoned"))?
            .insert(*execution_id, input.clone());
        Ok(())
    }

    async fn list(&self) -> Result<Vec<LlmBudget>, LlmBudgetError> {
        let mut budgets: Vec<LlmBudget> = self
            .budgets
            .lock()
            .map_err(|_| internal("budget store poisoned"))?
            .values()
            .cloned()
            .collect();
        budgets.sort_by(|a, b| b.label.cmp(&a.label));
        Ok(budgets)
    }

    async fn delete(&self, label: &str) -> Result<(), LlmBudgetError> {
        self.budgets
            .lock()
            .map_err(|_| internal("budget store poisoned"))?
            .remove(label);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_budget_round_trip() {
        let repo = InMemoryLlmBudgetRepository::new();
        let budget = LlmBudget {
            max_calls: 10,
            max_tokens: 1000,
            used_calls: 1,
            used_tokens: 50,
            label: "default".to_string(),
        };
        repo.save(&budget).await.unwrap();
        let loaded = repo.find_by_label("default").await.unwrap().unwrap();
        assert_eq!(loaded.used_calls, 1);
        assert_eq!(repo.list().await.unwrap().len(), 1);

        let exec_id = Uuid::new_v4();
        repo.record_reservation(
            &exec_id,
            &ReserveBudgetInput {
                execution_id: exec_id,
                estimated_tokens: 200,
                call_label: None,
            },
        )
        .await
        .unwrap();
        repo.record_commit(
            &exec_id,
            &CommitReservationInput {
                execution_id: exec_id,
                call_id: 1,
                reserved_tokens: 200,
                actual_tokens: 150,
            },
        )
        .await
        .unwrap();

        repo.delete("default").await.unwrap();
        assert!(repo.find_by_label("default").await.unwrap().is_none());
    }
}
