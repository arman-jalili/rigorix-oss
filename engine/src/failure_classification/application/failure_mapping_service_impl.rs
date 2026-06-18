//! Concrete implementation of `FailureMappingService`.
//!
//! @canonical .pi/architecture/modules/failure-classification.md#strategies
//! Implements: FailureMappingService — FailureType → RetryStrategy mapping
//! Issue: #34, #35 (RetryStrategy)
//!
//! Maps each `FailureType` to its recommended `RetryStrategy` per the
//! canonical mapping defined in the architecture module.

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::RwLock;

use crate::failure_classification::domain::{
    FailureClassificationError, FailureType, RetryStrategy,
};

use super::dto::{
    GetRetryStrategyInput, GetRetryStrategyOutput, StrategySource, ValidateConfigInput,
    ValidateConfigOutput, ValidationError,
};
use super::service::FailureMappingService;

/// Concrete implementation of `FailureMappingService`.
///
/// Maintains the default FailureType → RetryStrategy mapping and allows
/// registration of custom pattern-to-FailureType mappings.
pub struct FailureMappingServiceImpl {
    /// Custom pattern-to-FailureType mappings registered at runtime.
    custom_patterns: RwLock<HashMap<String, FailureType>>,
}

impl FailureMappingServiceImpl {
    /// Create a new `FailureMappingServiceImpl` with no custom patterns.
    pub fn new() -> Self {
        Self {
            custom_patterns: RwLock::new(HashMap::new()),
        }
    }
}

impl Default for FailureMappingServiceImpl {
    #[tracing::instrument(skip_all)]
    fn default() -> Self {
        Self::new()
    }
}

/// Returns the default mapping of FailureType → RetryStrategy.
pub fn default_mapping() -> HashMap<FailureType, RetryStrategy> {
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
    // NonRetryable has no meaningful strategy, but we include SameOperation
    // as a safe no-op fallback.
    map.insert(FailureType::NonRetryable, RetryStrategy::SameOperation);
    map
}

#[async_trait]
impl FailureMappingService for FailureMappingServiceImpl {
    async fn get_strategy(
        &self,
        input: GetRetryStrategyInput,
    ) -> Result<GetRetryStrategyOutput, FailureClassificationError> {
        // If an override is provided, use it
        if let Some(override_strategy) = &input.override_strategy {
            return Ok(GetRetryStrategyOutput {
                strategy: override_strategy.clone(),
                source: StrategySource::Override,
                description: format!(
                    "Override strategy for {:?}: {}",
                    input.failure_type,
                    override_strategy.description()
                ),
            });
        }

        // Look up in default mapping
        let mapping = default_mapping();
        let strategy = mapping.get(&input.failure_type).ok_or_else(|| {
            FailureClassificationError::MissingStrategy {
                failure_type: format!("{:?}", input.failure_type),
            }
        })?;

        Ok(GetRetryStrategyOutput {
            strategy: strategy.clone(),
            source: StrategySource::DefaultMapping,
            description: format!(
                "Default strategy for {:?}: {}",
                input.failure_type,
                strategy.description()
            ),
        })
    }

    async fn validate_config(
        &self,
        input: ValidateConfigInput,
    ) -> Result<ValidateConfigOutput, FailureClassificationError> {
        let mut errors: Vec<ValidationError> = Vec::new();
        let mut warnings: Vec<String> = Vec::new();

        // Validate custom patterns
        if let Some(patterns) = &input.custom_patterns {
            for (pattern, target) in patterns {
                if pattern.trim().is_empty() {
                    errors.push(ValidationError {
                        field: "custom_patterns".to_string(),
                        message: "Custom pattern must not be empty".to_string(),
                        value: Some(pattern.clone()),
                    });
                }
                if pattern.len() > 1024 {
                    errors.push(ValidationError {
                        field: "custom_patterns".to_string(),
                        message: "Custom pattern exceeds maximum length of 1024 characters"
                            .to_string(),
                        value: Some(format!("{}...", &pattern[..100])),
                    });
                }
                if matches!(target, FailureType::NonRetryable) {
                    warnings.push(format!(
                        "Pattern '{}' maps to NonRetryable — this is the default",
                        pattern
                    ));
                }
            }
        }

        // Validate custom strategy mappings
        if let Some(mappings) = &input.custom_strategy_mappings {
            for (failure_type, strategy) in mappings {
                if matches!(failure_type, FailureType::NonRetryable) {
                    warnings.push(format!(
                        "Mapping NonRetryable to {} — NonRetryable failures should not have a strategy",
                        strategy
                    ));
                }
                if let RetryStrategy::ExpandContext { level } = strategy
                    && *level > 5
                {
                    errors.push(ValidationError {
                        field: "custom_strategy_mappings".to_string(),
                        message: format!("ExpandContext level {} exceeds maximum of 5", level),
                        value: Some(level.to_string()),
                    });
                }
                if let RetryStrategy::PatchWithFeedback { feedback } = strategy
                    && feedback.is_empty()
                {
                    warnings.push(format!(
                        "PatchWithFeedback for {:?} has empty feedback string",
                        failure_type
                    ));
                }
            }
        }

        Ok(ValidateConfigOutput {
            valid: errors.is_empty(),
            errors,
            warnings,
        })
    }

    async fn register_pattern(
        &self,
        pattern: String,
        target: FailureType,
    ) -> Result<u32, FailureClassificationError> {
        if pattern.trim().is_empty() {
            return Err(FailureClassificationError::InvalidInput {
                detail: "Pattern must not be empty".to_string(),
            });
        }

        let mut patterns = self.custom_patterns.write().map_err(|e| {
            FailureClassificationError::PatternRepository {
                detail: format!("Lock poisoned: {}", e),
            }
        })?;

        patterns.insert(pattern.to_lowercase(), target);
        Ok(patterns.len() as u32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::failure_classification::application::dto::*;

    // -----------------------------------------------------------------------
    // get_strategy() tests
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_get_strategy_transient() {
        let service = FailureMappingServiceImpl::new();
        let input = GetRetryStrategyInput {
            failure_type: FailureType::Transient,
            override_strategy: None,
        };
        let output = service.get_strategy(input).await.unwrap();
        assert_eq!(output.strategy, RetryStrategy::SameOperation);
        assert_eq!(output.source, StrategySource::DefaultMapping);
    }

    #[tokio::test]
    async fn test_get_strategy_with_override() {
        let service = FailureMappingServiceImpl::new();
        let input = GetRetryStrategyInput {
            failure_type: FailureType::Transient,
            override_strategy: Some(RetryStrategy::Fallback),
        };
        let output = service.get_strategy(input).await.unwrap();
        assert_eq!(output.strategy, RetryStrategy::Fallback);
        assert_eq!(output.source, StrategySource::Override);
    }

    #[tokio::test]
    async fn test_get_strategy_lsp() {
        let service = FailureMappingServiceImpl::new();
        let input = GetRetryStrategyInput {
            failure_type: FailureType::LspConflict,
            override_strategy: None,
        };
        let output = service.get_strategy(input).await.unwrap();
        assert_eq!(output.strategy, RetryStrategy::ReExecute);
    }

    #[tokio::test]
    async fn test_get_strategy_resource_exhausted() {
        let service = FailureMappingServiceImpl::new();
        let input = GetRetryStrategyInput {
            failure_type: FailureType::ResourceExhausted,
            override_strategy: None,
        };
        let output = service.get_strategy(input).await.unwrap();
        assert_eq!(output.strategy, RetryStrategy::Fallback);
    }

    #[tokio::test]
    async fn test_get_strategy_system_error() {
        let service = FailureMappingServiceImpl::new();
        let input = GetRetryStrategyInput {
            failure_type: FailureType::SystemError,
            override_strategy: None,
        };
        let output = service.get_strategy(input).await.unwrap();
        assert_eq!(output.strategy, RetryStrategy::Fallback);
    }

    #[tokio::test]
    async fn test_get_strategy_test_failure() {
        let service = FailureMappingServiceImpl::new();
        let input = GetRetryStrategyInput {
            failure_type: FailureType::TestFailure,
            override_strategy: None,
        };
        let output = service.get_strategy(input).await.unwrap();
        assert!(matches!(
            output.strategy,
            RetryStrategy::PatchWithFeedback { .. }
        ));
        assert_eq!(output.source, StrategySource::DefaultMapping);
    }

    #[tokio::test]
    async fn test_get_strategy_build_failure() {
        let service = FailureMappingServiceImpl::new();
        let input = GetRetryStrategyInput {
            failure_type: FailureType::BuildFailure,
            override_strategy: None,
        };
        let output = service.get_strategy(input).await.unwrap();
        assert!(matches!(
            output.strategy,
            RetryStrategy::PatchWithFeedback { .. }
        ));
    }

    #[tokio::test]
    async fn test_get_strategy_description_provided() {
        let service = FailureMappingServiceImpl::new();
        let input = GetRetryStrategyInput {
            failure_type: FailureType::Transient,
            override_strategy: None,
        };
        let output = service.get_strategy(input).await.unwrap();
        assert!(!output.description.is_empty());
    }

    // -----------------------------------------------------------------------
    // register_pattern() tests
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_register_and_count() {
        let service = FailureMappingServiceImpl::new();
        let count = service
            .register_pattern("custom error".to_string(), FailureType::Transient)
            .await
            .unwrap();
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn test_register_multiple_patterns() {
        let service = FailureMappingServiceImpl::new();
        service
            .register_pattern("pattern1".to_string(), FailureType::Transient)
            .await
            .unwrap();
        let count = service
            .register_pattern("pattern2".to_string(), FailureType::LspConflict)
            .await
            .unwrap();
        assert_eq!(count, 2);
    }

    #[tokio::test]
    async fn test_register_empty_pattern_fails() {
        let service = FailureMappingServiceImpl::new();
        let result = service
            .register_pattern("".to_string(), FailureType::Transient)
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_register_whitespace_pattern_fails() {
        let service = FailureMappingServiceImpl::new();
        let result = service
            .register_pattern("   ".to_string(), FailureType::Transient)
            .await;
        assert!(result.is_err());
    }

    // -----------------------------------------------------------------------
    // validate_config() tests
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_validate_empty_config_is_valid() {
        let service = FailureMappingServiceImpl::new();
        let input = ValidateConfigInput {
            custom_patterns: None,
            custom_strategy_mappings: None,
        };
        let output = service.validate_config(input).await.unwrap();
        assert!(output.valid);
        assert!(output.errors.is_empty());
    }

    #[tokio::test]
    async fn test_validate_invalid_expansion_level() {
        let service = FailureMappingServiceImpl::new();
        let mut mappings = HashMap::new();
        mappings.insert(
            FailureType::Transient,
            RetryStrategy::ExpandContext { level: 10 },
        );
        let input = ValidateConfigInput {
            custom_patterns: None,
            custom_strategy_mappings: Some(mappings),
        };
        let output = service.validate_config(input).await.unwrap();
        assert!(!output.valid);
        assert!(!output.errors.is_empty());
    }

    #[tokio::test]
    async fn test_validate_non_retryable_mapping_warns() {
        let service = FailureMappingServiceImpl::new();
        let mut mappings = HashMap::new();
        mappings.insert(FailureType::NonRetryable, RetryStrategy::SameOperation);
        let input = ValidateConfigInput {
            custom_patterns: None,
            custom_strategy_mappings: Some(mappings),
        };
        let output = service.validate_config(input).await.unwrap();
        assert!(output.valid); // warnings only, not errors
        assert!(!output.warnings.is_empty());
    }

    #[tokio::test]
    async fn test_validate_empty_pattern_fails() {
        let service = FailureMappingServiceImpl::new();
        let mut patterns = HashMap::new();
        patterns.insert("".to_string(), FailureType::Transient);
        let input = ValidateConfigInput {
            custom_patterns: Some(patterns),
            custom_strategy_mappings: None,
        };
        let output = service.validate_config(input).await.unwrap();
        assert!(!output.valid);
    }

    #[tokio::test]
    async fn test_validate_oversized_pattern_fails() {
        let service = FailureMappingServiceImpl::new();
        let mut patterns = HashMap::new();
        patterns.insert("a".repeat(2000), FailureType::Transient);
        let input = ValidateConfigInput {
            custom_patterns: Some(patterns),
            custom_strategy_mappings: None,
        };
        let output = service.validate_config(input).await.unwrap();
        assert!(!output.valid);
    }

    // -----------------------------------------------------------------------
    // default_mapping() tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_default_mapping_contains_all_types() {
        let mapping = default_mapping();
        assert!(mapping.contains_key(&FailureType::Transient));
        assert!(mapping.contains_key(&FailureType::LspConflict));
        assert!(mapping.contains_key(&FailureType::ResourceExhausted));
        assert!(mapping.contains_key(&FailureType::SystemError));
        assert!(mapping.contains_key(&FailureType::TestFailure));
        assert!(mapping.contains_key(&FailureType::BuildFailure));
        assert!(mapping.contains_key(&FailureType::NonRetryable));
        assert_eq!(mapping.len(), 7);
    }
}
