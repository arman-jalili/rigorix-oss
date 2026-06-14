//! Implementation of the LlmBudgetFactory.
//!
//! @canonical .pi/architecture/modules/budget-tracking.md#application
//! Implements: ISSUE-BUDGET-TRACKING-1 — LlmBudgetFactory
//! Issue: #69
//!
//! Provides the concrete factory for constructing `LlmBudgetImpl` instances
//! from preset modes (default, advanced, aggressive) or custom limits.

use async_trait::async_trait;

use crate::budget_tracking::application::llm_budget_impl::LlmBudgetImpl;
use crate::budget_tracking::application::factory::LlmBudgetFactory;
use crate::budget_tracking::application::service::LlmBudgetService;
use crate::budget_tracking::domain::LlmBudgetError;

/// Factory for creating `LlmBudgetImpl` instances.
///
/// Constructs budgets from preset modes with documented default limits.
pub struct LlmBudgetFactoryImpl;

#[async_trait]
impl LlmBudgetFactory for LlmBudgetFactoryImpl {
    /// Default mode: 5 calls, 10K tokens.
    async fn create_default(&self) -> Result<Box<dyn LlmBudgetService>, LlmBudgetError> {
        Ok(Box::new(LlmBudgetImpl::new(
            5,
            10_000,
            "default".to_string(),
        )))
    }

    /// Advanced mode: 20 calls, 100K tokens.
    async fn create_advanced(&self) -> Result<Box<dyn LlmBudgetService>, LlmBudgetError> {
        Ok(Box::new(LlmBudgetImpl::new(
            20,
            100_000,
            "advanced".to_string(),
        )))
    }

    /// Aggressive mode: 50 calls, 500K tokens.
    async fn create_aggressive(&self) -> Result<Box<dyn LlmBudgetService>, LlmBudgetError> {
        Ok(Box::new(LlmBudgetImpl::new(
            50,
            500_000,
            "aggressive".to_string(),
        )))
    }

    /// Custom mode: arbitrary limits.
    async fn create_custom(
        &self,
        max_calls: u32,
        max_tokens: u32,
        label: String,
    ) -> Result<Box<dyn LlmBudgetService>, LlmBudgetError> {
        if max_calls == 0 {
            return Err(LlmBudgetError::NotInitialized {
                detail: "max_calls must be > 0".to_string(),
            });
        }
        if max_tokens == 0 {
            return Err(LlmBudgetError::NotInitialized {
                detail: "max_tokens must be > 0".to_string(),
            });
        }
        Ok(Box::new(LlmBudgetImpl::new(max_calls, max_tokens, label)))
    }

    /// Create from enforcement config values.
    async fn create_from_enforcement_config(
        &self,
        max_tool_calls: u64,
        max_tokens: u64,
    ) -> Result<Box<dyn LlmBudgetService>, LlmBudgetError> {
        let max_calls = u32::try_from(max_tool_calls).unwrap_or(u32::MAX);
        let max_tok = u32::try_from(max_tokens).unwrap_or(u32::MAX);

        if max_calls == 0 {
            return Err(LlmBudgetError::NotInitialized {
                detail: "enforcement config has zero max_tool_calls".to_string(),
            });
        }
        if max_tok == 0 {
            return Err(LlmBudgetError::NotInitialized {
                detail: "enforcement config has zero max_tokens".to_string(),
            });
        }

        Ok(Box::new(LlmBudgetImpl::new(
            max_calls,
            max_tok,
            "enforcement".to_string(),
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_factory_default() {
        let factory = LlmBudgetFactoryImpl;
        let budget = factory.create_default().await.unwrap();
        let status = budget
            .get_status(crate::budget_tracking::application::dto::GetBudgetStatusInput {
                execution_id: uuid::Uuid::new_v4(),
            })
            .await
            .unwrap();
        assert_eq!(status.max_calls, 5);
        assert_eq!(status.max_tokens, 10_000);
        assert_eq!(status.label, "default");
    }

    #[tokio::test]
    async fn test_factory_advanced() {
        let factory = LlmBudgetFactoryImpl;
        let budget = factory.create_advanced().await.unwrap();
        let status = budget
            .get_status(crate::budget_tracking::application::dto::GetBudgetStatusInput {
                execution_id: uuid::Uuid::new_v4(),
            })
            .await
            .unwrap();
        assert_eq!(status.max_calls, 20);
        assert_eq!(status.max_tokens, 100_000);
        assert_eq!(status.label, "advanced");
    }

    #[tokio::test]
    async fn test_factory_aggressive() {
        let factory = LlmBudgetFactoryImpl;
        let budget = factory.create_aggressive().await.unwrap();
        let status = budget
            .get_status(crate::budget_tracking::application::dto::GetBudgetStatusInput {
                execution_id: uuid::Uuid::new_v4(),
            })
            .await
            .unwrap();
        assert_eq!(status.max_calls, 50);
        assert_eq!(status.max_tokens, 500_000);
        assert_eq!(status.label, "aggressive");
    }

    #[tokio::test]
    async fn test_factory_custom() {
        let factory = LlmBudgetFactoryImpl;
        let budget = factory.create_custom(100, 1_000_000, "custom".to_string()).await.unwrap();
        let status = budget
            .get_status(crate::budget_tracking::application::dto::GetBudgetStatusInput {
                execution_id: uuid::Uuid::new_v4(),
            })
            .await
            .unwrap();
        assert_eq!(status.max_calls, 100);
        assert_eq!(status.max_tokens, 1_000_000);
        assert_eq!(status.label, "custom");
    }

    #[tokio::test]
    async fn test_factory_custom_zero_calls_fails() {
        let factory = LlmBudgetFactoryImpl;
        let result = factory.create_custom(0, 10_000, "bad".to_string()).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_factory_from_enforcement() {
        let factory = LlmBudgetFactoryImpl;
        let budget = factory
            .create_from_enforcement_config(500, 100_000)
            .await
            .unwrap();
        let status = budget
            .get_status(crate::budget_tracking::application::dto::GetBudgetStatusInput {
                execution_id: uuid::Uuid::new_v4(),
            })
            .await
            .unwrap();
        assert_eq!(status.max_calls, 500);
        assert_eq!(status.max_tokens, 100_000);
    }
}
