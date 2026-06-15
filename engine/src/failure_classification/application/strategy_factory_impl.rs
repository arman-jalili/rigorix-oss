//! Concrete implementation of `StrategyFactory`.
//!
//! @canonical .pi/architecture/modules/failure-classification.md#strategies
//! Implements: StrategyFactory — RetryStrategy construction with validation
//! Issue: #34, #35 (RetryStrategy)
//!
//! Builds `RetryStrategy` instances with proper validation (e.g., ensuring
//! `ExpandContext` levels are within range, `PatchWithFeedback` has content).

use async_trait::async_trait;
use std::collections::HashMap;

use crate::failure_classification::domain::{
    FailureClassificationError, FailureType, RetryStrategy,
};

use super::factory::StrategyFactory;

/// Concrete implementation of `StrategyFactory`.
///
/// Validates strategy parameters before construction:
/// - `ExpandContext` level must be 0–5
/// - `PatchWithFeedback` feedback must be non-empty
pub struct StrategyFactoryImpl;

#[async_trait]
impl StrategyFactory for StrategyFactoryImpl {
    async fn create_patch_with_feedback(
        &self,
        feedback: &str,
    ) -> Result<RetryStrategy, FailureClassificationError> {
        if feedback.trim().is_empty() {
            return Err(FailureClassificationError::InvalidInput {
                detail: "PatchWithFeedback feedback must not be empty".to_string(),
            });
        }
        Ok(RetryStrategy::PatchWithFeedback {
            feedback: feedback.to_string(),
        })
    }

    async fn create_expand_context(
        &self,
        level: u8,
    ) -> Result<RetryStrategy, FailureClassificationError> {
        if level > 5 {
            return Err(FailureClassificationError::InvalidExpansionLevel {
                level,
                min: 0,
                max: 5,
            });
        }
        Ok(RetryStrategy::ExpandContext { level })
    }

    #[tracing::instrument(skip_all)]
    fn create_same_operation(&self) -> RetryStrategy {
        RetryStrategy::SameOperation
    }

    #[tracing::instrument(skip_all)]
    fn create_re_execute(&self) -> RetryStrategy {
        RetryStrategy::ReExecute
    }

    #[tracing::instrument(skip_all)]
    fn create_fallback(&self) -> RetryStrategy {
        RetryStrategy::Fallback
    }

    #[tracing::instrument(skip_all)]
    fn build_default_mapping(&self) -> HashMap<FailureType, RetryStrategy> {
        let mut map = HashMap::new();
        map.insert(FailureType::Transient, RetryStrategy::SameOperation);
        map.insert(FailureType::LspConflict, RetryStrategy::ReExecute);
        map.insert(FailureType::ResourceExhausted, RetryStrategy::Fallback);
        map.insert(FailureType::SystemError, RetryStrategy::Fallback);
        map.insert(
            FailureType::TestFailure,
            RetryStrategy::PatchWithFeedback {
                feedback: String::new(),
            },
        );
        map.insert(
            FailureType::BuildFailure,
            RetryStrategy::PatchWithFeedback {
                feedback: String::new(),
            },
        );
        map.insert(FailureType::NonRetryable, RetryStrategy::SameOperation);
        map
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_same_operation() {
        let factory = StrategyFactoryImpl;
        assert_eq!(
            factory.create_same_operation(),
            RetryStrategy::SameOperation
        );
    }

    #[tokio::test]
    async fn test_create_re_execute() {
        let factory = StrategyFactoryImpl;
        assert_eq!(factory.create_re_execute(), RetryStrategy::ReExecute);
    }

    #[tokio::test]
    async fn test_create_fallback() {
        let factory = StrategyFactoryImpl;
        assert_eq!(factory.create_fallback(), RetryStrategy::Fallback);
    }

    #[tokio::test]
    async fn test_create_patch_with_feedback_valid() {
        let factory = StrategyFactoryImpl;
        let strategy = factory
            .create_patch_with_feedback("build error: undefined reference")
            .await
            .unwrap();
        assert!(matches!(strategy, RetryStrategy::PatchWithFeedback { .. }));
        if let RetryStrategy::PatchWithFeedback { feedback } = strategy {
            assert_eq!(feedback, "build error: undefined reference");
        }
    }

    #[tokio::test]
    async fn test_create_patch_with_feedback_empty_fails() {
        let factory = StrategyFactoryImpl;
        let result = factory.create_patch_with_feedback("").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_create_expand_context_valid() {
        let factory = StrategyFactoryImpl;
        let strategy = factory.create_expand_context(3).await.unwrap();
        assert!(matches!(
            strategy,
            RetryStrategy::ExpandContext { level: 3 }
        ));
    }

    #[tokio::test]
    async fn test_create_expand_context_zero() {
        let factory = StrategyFactoryImpl;
        let strategy = factory.create_expand_context(0).await.unwrap();
        assert!(matches!(
            strategy,
            RetryStrategy::ExpandContext { level: 0 }
        ));
    }

    #[tokio::test]
    async fn test_create_expand_context_max() {
        let factory = StrategyFactoryImpl;
        let strategy = factory.create_expand_context(5).await.unwrap();
        assert!(matches!(
            strategy,
            RetryStrategy::ExpandContext { level: 5 }
        ));
    }

    #[tokio::test]
    async fn test_create_expand_context_too_high_fails() {
        let factory = StrategyFactoryImpl;
        let result = factory.create_expand_context(6).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            FailureClassificationError::InvalidExpansionLevel { level, min, max } => {
                assert_eq!(level, 6);
                assert_eq!(min, 0);
                assert_eq!(max, 5);
            }
            _ => panic!("Expected InvalidExpansionLevel error"),
        }
    }

    #[tokio::test]
    async fn test_build_default_mapping() {
        let factory = StrategyFactoryImpl;
        let mapping = factory.build_default_mapping();
        assert_eq!(mapping.len(), 7);
        assert!(mapping.contains_key(&FailureType::Transient));
        assert!(mapping.contains_key(&FailureType::LspConflict));
        assert!(mapping.contains_key(&FailureType::ResourceExhausted));
        assert!(mapping.contains_key(&FailureType::SystemError));
        assert!(mapping.contains_key(&FailureType::TestFailure));
        assert!(mapping.contains_key(&FailureType::BuildFailure));
        assert!(mapping.contains_key(&FailureType::NonRetryable));
    }

    #[tokio::test]
    async fn test_default_mapping_values() {
        let factory = StrategyFactoryImpl;
        let mapping = factory.build_default_mapping();
        assert_eq!(
            mapping.get(&FailureType::Transient).unwrap(),
            &RetryStrategy::SameOperation
        );
        assert_eq!(
            mapping.get(&FailureType::LspConflict).unwrap(),
            &RetryStrategy::ReExecute
        );
        assert_eq!(
            mapping.get(&FailureType::ResourceExhausted).unwrap(),
            &RetryStrategy::Fallback
        );
    }
}
