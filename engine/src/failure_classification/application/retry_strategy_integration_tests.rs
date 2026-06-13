//! Integration tests for RetryStrategy — end-to-end strategy flows.
//!
//! @canonical .pi/architecture/modules/failure-classification.md#strategies
//! Implements: RetryStrategy integration — full FailureType→Strategy pipeline
//! Issue: #35
//!
//! Tests the complete flow: classify failure → get strategy → verify eligibility

#[cfg(test)]
mod tests {
    use crate::failure_classification::application::*;
    use crate::failure_classification::domain::*;

    // -----------------------------------------------------------------------
    // Full pipeline: classify → map → eligibility
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_full_pipeline_transient() {
        let classifier = FailureClassifierServiceImpl;
        let mapper = FailureMappingServiceImpl::new();

        // Step 1: Classify
        let classify_input = dto::ClassifyFailureInput {
            error_message: "connection timed out".to_string(),
            operation_context: None,
            source: None,
        };
        let classify_output = classifier.classify(classify_input).await.unwrap();
        assert_eq!(classify_output.failure_type, FailureType::Transient);

        // Step 2: Get strategy
        let strategy_input = dto::GetRetryStrategyInput {
            failure_type: classify_output.failure_type.clone(),
            override_strategy: None,
        };
        let strategy_output = mapper.get_strategy(strategy_input).await.unwrap();
        assert_eq!(strategy_output.strategy, RetryStrategy::SameOperation);

        // Step 3: Check eligibility
        let eligibility_input = dto::CheckRetryEligibilityInput {
            failure_type: classify_output.failure_type.clone(),
            current_retry_count: Some(0),
            max_retries: Some(3),
        };
        let eligibility_output = classifier
            .check_retry_eligibility(eligibility_input)
            .await
            .unwrap();
        assert!(eligibility_output.eligible);
    }

    #[tokio::test]
    async fn test_full_pipeline_test_failure() {
        let classifier = FailureClassifierServiceImpl;
        let mapper = FailureMappingServiceImpl::new();

        let classify_input = dto::ClassifyFailureInput {
            error_message: "tests failed with 3 errors".to_string(),
            operation_context: None,
            source: None,
        };
        let classify_output = classifier.classify(classify_input).await.unwrap();
        assert_eq!(classify_output.failure_type, FailureType::TestFailure);

        let strategy_input = dto::GetRetryStrategyInput {
            failure_type: classify_output.failure_type.clone(),
            override_strategy: None,
        };
        let strategy_output = mapper.get_strategy(strategy_input).await.unwrap();
        assert!(matches!(
            strategy_output.strategy,
            RetryStrategy::PatchWithFeedback { .. }
        ));

        let eligibility_input = dto::CheckRetryEligibilityInput {
            failure_type: classify_output.failure_type.clone(),
            current_retry_count: Some(0),
            max_retries: Some(3),
        };
        let eligibility_output = classifier
            .check_retry_eligibility(eligibility_input)
            .await
            .unwrap();
        assert!(!eligibility_output.eligible);
    }

    #[tokio::test]
    async fn test_full_pipeline_build_failure() {
        let classifier = FailureClassifierServiceImpl;
        let mapper = FailureMappingServiceImpl::new();

        let classify_input = dto::ClassifyFailureInput {
            error_message: "build error: cannot compile package".to_string(),
            operation_context: None,
            source: None,
        };
        let classify_output = classifier.classify(classify_input).await.unwrap();
        assert_eq!(classify_output.failure_type, FailureType::BuildFailure);

        let strategy_input = dto::GetRetryStrategyInput {
            failure_type: classify_output.failure_type.clone(),
            override_strategy: None,
        };
        let strategy_output = mapper.get_strategy(strategy_input).await.unwrap();
        assert!(matches!(
            strategy_output.strategy,
            RetryStrategy::PatchWithFeedback { .. }
        ));
    }

    #[tokio::test]
    async fn test_full_pipeline_lsp() {
        let classifier = FailureClassifierServiceImpl;
        let mapper = FailureMappingServiceImpl::new();

        let classify_input = dto::ClassifyFailureInput {
            error_message: "LSP type mismatch: expected String".to_string(),
            operation_context: None,
            source: None,
        };
        let classify_output = classifier.classify(classify_input).await.unwrap();
        assert_eq!(classify_output.failure_type, FailureType::LspConflict);

        let strategy_input = dto::GetRetryStrategyInput {
            failure_type: classify_output.failure_type.clone(),
            override_strategy: None,
        };
        let strategy_output = mapper.get_strategy(strategy_input).await.unwrap();
        assert_eq!(strategy_output.strategy, RetryStrategy::ReExecute);
    }

    #[tokio::test]
    async fn test_full_pipeline_resource_exhausted() {
        let classifier = FailureClassifierServiceImpl;
        let mapper = FailureMappingServiceImpl::new();

        let classify_input = dto::ClassifyFailureInput {
            error_message: "out of memory error".to_string(),
            operation_context: None,
            source: None,
        };
        let classify_output = classifier.classify(classify_input).await.unwrap();
        assert_eq!(
            classify_output.failure_type,
            FailureType::ResourceExhausted
        );

        let strategy_input = dto::GetRetryStrategyInput {
            failure_type: classify_output.failure_type.clone(),
            override_strategy: None,
        };
        let strategy_output = mapper.get_strategy(strategy_input).await.unwrap();
        assert_eq!(strategy_output.strategy, RetryStrategy::Fallback);
    }

    #[tokio::test]
    async fn test_full_pipeline_system_error() {
        let classifier = FailureClassifierServiceImpl;
        let mapper = FailureMappingServiceImpl::new();

        let classify_input = dto::ClassifyFailureInput {
            error_message: "process crash: signal 11".to_string(),
            operation_context: None,
            source: None,
        };
        let classify_output = classifier.classify(classify_input).await.unwrap();
        assert_eq!(classify_output.failure_type, FailureType::SystemError);

        let strategy_input = dto::GetRetryStrategyInput {
            failure_type: classify_output.failure_type.clone(),
            override_strategy: None,
        };
        let strategy_output = mapper.get_strategy(strategy_input).await.unwrap();
        assert_eq!(strategy_output.strategy, RetryStrategy::Fallback);
    }

    #[tokio::test]
    async fn test_full_pipeline_non_retryable() {
        let classifier = FailureClassifierServiceImpl;
        let mapper = FailureMappingServiceImpl::new();

        let classify_input = dto::ClassifyFailureInput {
            error_message: "invalid api key".to_string(),
            operation_context: None,
            source: None,
        };
        let classify_output = classifier.classify(classify_input).await.unwrap();
        assert_eq!(classify_output.failure_type, FailureType::NonRetryable);

        let strategy_input = dto::GetRetryStrategyInput {
            failure_type: classify_output.failure_type.clone(),
            override_strategy: Some(RetryStrategy::Fallback),
        };
        let strategy_output = mapper.get_strategy(strategy_input).await.unwrap();
        assert_eq!(strategy_output.strategy, RetryStrategy::Fallback);
        assert_eq!(
            strategy_output.source,
            dto::StrategySource::Override
        );
    }

    // -----------------------------------------------------------------------
    // Strategy override flows
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_strategy_override_changes_behavior() {
        let mapper = FailureMappingServiceImpl::new();

        // Default strategy for Transient
        let default_input = dto::GetRetryStrategyInput {
            failure_type: FailureType::Transient,
            override_strategy: None,
        };
        let default_output = mapper.get_strategy(default_input).await.unwrap();
        assert_eq!(default_output.strategy, RetryStrategy::SameOperation);
        assert_eq!(default_output.source, dto::StrategySource::DefaultMapping);

        // Override with Fallback
        let override_input = dto::GetRetryStrategyInput {
            failure_type: FailureType::Transient,
            override_strategy: Some(RetryStrategy::Fallback),
        };
        let override_output = mapper.get_strategy(override_input).await.unwrap();
        assert_eq!(override_output.strategy, RetryStrategy::Fallback);
        assert_eq!(override_output.source, dto::StrategySource::Override);
    }

    #[tokio::test]
    async fn test_override_description_reflects_strategy() {
        let mapper = FailureMappingServiceImpl::new();
        let input = dto::GetRetryStrategyInput {
            failure_type: FailureType::TestFailure,
            override_strategy: Some(RetryStrategy::ReExecute),
        };
        let output = mapper.get_strategy(input).await.unwrap();
        assert!(output.description.contains("Override"));
        assert!(output.description.contains("Re-execute from scratch"));
    }

    // -----------------------------------------------------------------------
    // Retry eligibility with strategy context
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_eligibility_with_varying_retry_counts() {
        let classifier = FailureClassifierServiceImpl;

        let test_cases = vec![
            (0u32, true, 3u32),  // fresh → eligible
            (1, true, 2),         // one retry → eligible
            (2, true, 1),         // two retries → eligible
            (3, false, 0),        // max reached → not eligible
            (5, false, 0),        // over limit → not eligible
        ];

        for (current, expect_eligible, expect_remaining) in test_cases {
            let input = dto::CheckRetryEligibilityInput {
                failure_type: FailureType::Transient,
                current_retry_count: Some(current),
                max_retries: Some(3),
            };
            let output = classifier.check_retry_eligibility(input).await.unwrap();
            assert_eq!(
                output.eligible, expect_eligible,
                "Expected eligible={} for retry_count={}",
                expect_eligible, current
            );
            assert_eq!(
                output.remaining_attempts,
                Some(expect_remaining),
                "Expected {} remaining for retry_count={}",
                expect_remaining, current
            );
        }
    }

    // -----------------------------------------------------------------------
    // Strategy factory flows
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_factory_default_mapping_consistency() {
        let factory = StrategyFactoryImpl;
        let mapping = factory.build_default_mapping();

        // Verify every FailureType has a mapping
        for ft in &[
            FailureType::Transient,
            FailureType::TestFailure,
            FailureType::BuildFailure,
            FailureType::LspConflict,
            FailureType::ResourceExhausted,
            FailureType::SystemError,
            FailureType::NonRetryable,
        ] {
            assert!(
                mapping.contains_key(ft),
                "Missing default strategy for {:?}",
                ft
            );
        }
    }

    #[tokio::test]
    async fn test_factory_expand_context_boundaries() {
        let factory = StrategyFactoryImpl;

        // Valid levels
        for level in 0..=5 {
            let strategy = factory.create_expand_context(level).await.unwrap();
            assert!(matches!(strategy, RetryStrategy::ExpandContext { level: l } if l == level));
        }

        // Invalid levels
        for level in [6, 10, 255] {
            let result = factory.create_expand_context(level).await;
            assert!(result.is_err(), "Level {} should be rejected", level);
        }
    }

    #[tokio::test]
    async fn test_factory_patch_with_feedback() {
        let factory = StrategyFactoryImpl;

        // Empty feedback should fail
        assert!(factory.create_patch_with_feedback("").await.is_err());
        assert!(factory.create_patch_with_feedback("   ").await.is_err());

        // Valid feedback
        let strategy = factory
            .create_patch_with_feedback("error: undefined reference to 'main'")
            .await
            .unwrap();
        if let RetryStrategy::PatchWithFeedback { feedback } = strategy {
            assert_eq!(feedback, "error: undefined reference to 'main'");
        } else {
            panic!("Expected PatchWithFeedback");
        }
    }
}
