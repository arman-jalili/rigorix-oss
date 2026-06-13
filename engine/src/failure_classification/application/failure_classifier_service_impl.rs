//! Concrete implementation of `FailureClassifierService`.
//!
//! @canonical .pi/architecture/modules/failure-classification.md#classifier
//! Implements: FailureClassifierService — pattern-matching failure classification
//! Issue: #34
//!
//! Performs case-insensitive pattern matching against error messages
//! to determine the appropriate `FailureType`. Uses the built-in patterns
//! defined in the architecture module.

use async_trait::async_trait;

use crate::failure_classification::domain::{
    FailureClassificationError, FailureType, RetryStrategy,
};

use super::dto::{
    CheckRetryEligibilityInput, CheckRetryEligibilityOutput, ClassifyFailureInput,
    ClassifyFailureOutput,
};
use super::service::FailureClassifierService;

/// Default maximum number of retry attempts.
const DEFAULT_MAX_RETRIES: u32 = 3;

/// Concrete implementation of `FailureClassifierService`.
///
/// Classifies error messages by matching against known patterns
/// in order of specificity. If no pattern matches, returns `NonRetryable`.
pub struct FailureClassifierServiceImpl;

#[async_trait]
impl FailureClassifierService for FailureClassifierServiceImpl {
    async fn classify(
        &self,
        input: ClassifyFailureInput,
    ) -> Result<ClassifyFailureOutput, FailureClassificationError> {
        let error_message = input.error_message.trim();

        if error_message.is_empty() {
            return Err(FailureClassificationError::InvalidInput {
                detail: "Error message must not be empty".to_string(),
            });
        }

        let error_lower = error_message.to_lowercase();
        let context_lower = input
            .operation_context
            .as_deref()
            .unwrap_or("")
            .to_lowercase();

        let (failure_type, explanation) = classify_internal(&error_lower, &context_lower);
        let is_retryable = failure_type.is_retryable();
        let recommended_strategy = default_strategy_for(&failure_type);

        Ok(ClassifyFailureOutput {
            failure_type,
            recommended_strategy,
            is_retryable,
            confidence: Some(0.9),
            explanation: Some(explanation),
        })
    }

    async fn classify_type(
        &self,
        error_message: &str,
    ) -> Result<FailureType, FailureClassificationError> {
        if error_message.trim().is_empty() {
            return Err(FailureClassificationError::InvalidInput {
                detail: "Error message must not be empty".to_string(),
            });
        }

        let error_lower = error_message.to_lowercase();
        let (failure_type, _) = classify_internal(&error_lower, "");
        Ok(failure_type)
    }

    async fn check_retry_eligibility(
        &self,
        input: CheckRetryEligibilityInput,
    ) -> Result<CheckRetryEligibilityOutput, FailureClassificationError> {
        let inherently_retryable = input.failure_type.is_retryable();
        let max_retries = input.max_retries.unwrap_or(DEFAULT_MAX_RETRIES);
        let current_count = input.current_retry_count.unwrap_or(0);

        if !inherently_retryable {
            return Ok(CheckRetryEligibilityOutput {
                eligible: false,
                reason: format!(
                    "{:?} is not inherently retryable",
                    input.failure_type
                ),
                remaining_attempts: None,
            });
        }

        if current_count >= max_retries {
            return Ok(CheckRetryEligibilityOutput {
                eligible: false,
                reason: format!(
                    "Retry limit reached: {} of {} attempts exhausted",
                    current_count, max_retries
                ),
                remaining_attempts: Some(0),
            });
        }

        let remaining = max_retries.saturating_sub(current_count);
        Ok(CheckRetryEligibilityOutput {
            eligible: true,
            reason: format!("Eligible for retry ({} of {} remaining)", remaining, max_retries),
            remaining_attempts: Some(remaining),
        })
    }
}

/// Internal classification function.
///
/// Checks patterns in order of specificity:
/// 1. Resource/system errors (most specific keywords)
/// 2. Build/test failures
/// 3. LSP conflicts
/// 4. Transient errors
/// 5. Default to NonRetryable
fn classify_internal(error_lower: &str, context_lower: &str) -> (FailureType, String) {
    // Check for resource exhaustion
    if contains_any(error_lower, &["out of memory", "oom", "disk full", "no space"])
        || contains_any(context_lower, &["out of memory", "oom", "disk full"])
    {
        return (
            FailureType::ResourceExhausted,
            "Matched resource exhaustion pattern (OOM/disk full)".to_string(),
        );
    }

    // Check for system errors
    if contains_any(
        error_lower,
        &[
            "signal",
            "process crash",
            "killed",
            "segmentation fault",
            "segfault",
            "core dump",
            "io error",
            "broken pipe",
            "connection reset",
        ],
    ) {
        return (
            FailureType::SystemError,
            "Matched system error pattern (process crash/I/O)".to_string(),
        );
    }

    // Check for build failure
    if contains_any(
        error_lower,
        &[
            "build fail",
            "compile error",
            "compilation error",
            "build error",
            "cannot compile",
            "build failed",
        ],
    ) {
        return (
            FailureType::BuildFailure,
            "Matched build failure pattern".to_string(),
        );
    }

    // Check for test failure
    if contains_any(error_lower, &["test fail", "test error", "tests failed"])
        || (contains_any(error_lower, &["fail", "error"])
            && contains_any(context_lower, &["test", "testing"]))
    {
        return (
            FailureType::TestFailure,
            "Matched test failure pattern".to_string(),
        );
    }

    // Check for LSP conflicts
    if contains_any(
        error_lower,
        &[
            "lsp",
            "type error",
            "type mismatch",
            "type conflict",
            "cannot find type",
            "does not implement",
        ],
    ) {
        return (
            FailureType::LspConflict,
            "Matched LSP/type conflict pattern".to_string(),
        );
    }

    // Check for transient errors
    if contains_any(
        error_lower,
        &[
            "timeout",
            "timed out",
            "connection",
            "network",
            "refused",
            "rate limit",
            "too many requests",
            "429",
            "503",
            "502",
            "504",
            "temporary",
            "retry",
        ],
    ) {
        return (
            FailureType::Transient,
            "Matched transient error pattern (timeout/network)".to_string(),
        );
    }

    // Nothing matched — NonRetryable
    (
        FailureType::NonRetryable,
        "No matching pattern found — classified as NonRetryable".to_string(),
    )
}

/// Returns the default RetryStrategy for a given FailureType.
pub fn default_strategy_for(failure_type: &FailureType) -> RetryStrategy {
    match failure_type {
        FailureType::Transient => RetryStrategy::SameOperation,
        FailureType::LspConflict => RetryStrategy::ReExecute,
        FailureType::ResourceExhausted => RetryStrategy::Fallback,
        FailureType::SystemError => RetryStrategy::Fallback,
        FailureType::TestFailure => RetryStrategy::PatchWithFeedback {
            feedback: String::new(),
        },
        FailureType::BuildFailure => RetryStrategy::PatchWithFeedback {
            feedback: String::new(),
        },
        FailureType::NonRetryable => RetryStrategy::SameOperation, // fallback, won't be used
    }
}

/// Returns `true` if `text` contains any of the given substrings.
fn contains_any(text: &str, patterns: &[&str]) -> bool {
    patterns.iter().any(|p| text.contains(p))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::failure_classification::application::dto::*;

    // -----------------------------------------------------------------------
    // classify() tests
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_classify_transient_timeout() {
        let service = FailureClassifierServiceImpl;
        let input = ClassifyFailureInput {
            error_message: "connection timed out".to_string(),
            operation_context: None,
            source: None,
        };
        let output = service.classify(input).await.unwrap();
        assert_eq!(output.failure_type, FailureType::Transient);
        assert!(output.is_retryable);
        assert_eq!(output.recommended_strategy, RetryStrategy::SameOperation);
    }

    #[tokio::test]
    async fn test_classify_transient_network() {
        let service = FailureClassifierServiceImpl;
        let input = ClassifyFailureInput {
            error_message: "network error: connection refused".to_string(),
            operation_context: None,
            source: None,
        };
        let output = service.classify(input).await.unwrap();
        assert_eq!(output.failure_type, FailureType::Transient);
        assert!(output.is_retryable);
    }

    #[tokio::test]
    async fn test_classify_transient_rate_limit() {
        let service = FailureClassifierServiceImpl;
        let input = ClassifyFailureInput {
            error_message: "rate limit exceeded: 429 Too Many Requests".to_string(),
            operation_context: None,
            source: None,
        };
        let output = service.classify(input).await.unwrap();
        assert_eq!(output.failure_type, FailureType::Transient);
    }

    #[tokio::test]
    async fn test_classify_test_failure() {
        let service = FailureClassifierServiceImpl;
        let input = ClassifyFailureInput {
            error_message: "tests failed with 3 errors".to_string(),
            operation_context: None,
            source: None,
        };
        let output = service.classify(input).await.unwrap();
        assert_eq!(output.failure_type, FailureType::TestFailure);
        assert!(!output.is_retryable);
        assert!(matches!(
            output.recommended_strategy,
            RetryStrategy::PatchWithFeedback { .. }
        ));
    }

    #[tokio::test]
    async fn test_classify_build_failure() {
        let service = FailureClassifierServiceImpl;
        let input = ClassifyFailureInput {
            error_message: "build failed with compile error in src/main.rs".to_string(),
            operation_context: None,
            source: None,
        };
        let output = service.classify(input).await.unwrap();
        assert_eq!(output.failure_type, FailureType::BuildFailure);
        assert!(!output.is_retryable);
    }

    #[tokio::test]
    async fn test_classify_lsp_conflict() {
        let service = FailureClassifierServiceImpl;
        let input = ClassifyFailureInput {
            error_message: "LSP type mismatch: expected String, found i32".to_string(),
            operation_context: None,
            source: None,
        };
        let output = service.classify(input).await.unwrap();
        assert_eq!(output.failure_type, FailureType::LspConflict);
        assert!(output.is_retryable);
        assert_eq!(output.recommended_strategy, RetryStrategy::ReExecute);
    }

    #[tokio::test]
    async fn test_classify_resource_exhausted() {
        let service = FailureClassifierServiceImpl;
        let input = ClassifyFailureInput {
            error_message: "out of memory: cannot allocate 1GB".to_string(),
            operation_context: None,
            source: None,
        };
        let output = service.classify(input).await.unwrap();
        assert_eq!(output.failure_type, FailureType::ResourceExhausted);
        assert!(output.is_retryable);
        assert_eq!(output.recommended_strategy, RetryStrategy::Fallback);
    }

    #[tokio::test]
    async fn test_classify_system_error() {
        let service = FailureClassifierServiceImpl;
        let input = ClassifyFailureInput {
            error_message: "process killed: signal 9".to_string(),
            operation_context: None,
            source: None,
        };
        let output = service.classify(input).await.unwrap();
        assert_eq!(output.failure_type, FailureType::SystemError);
        assert!(output.is_retryable);
        assert_eq!(output.recommended_strategy, RetryStrategy::Fallback);
    }

    #[tokio::test]
    async fn test_classify_non_retryable() {
        let service = FailureClassifierServiceImpl;
        let input = ClassifyFailureInput {
            error_message: "invalid api key: authentication failed".to_string(),
            operation_context: None,
            source: None,
        };
        let output = service.classify(input).await.unwrap();
        assert_eq!(output.failure_type, FailureType::NonRetryable);
        assert!(!output.is_retryable);
    }

    #[tokio::test]
    async fn test_classify_empty_message_fails() {
        let service = FailureClassifierServiceImpl;
        let input = ClassifyFailureInput {
            error_message: "   ".to_string(),
            operation_context: None,
            source: None,
        };
        let result = service.classify(input).await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            FailureClassificationError::InvalidInput { .. }
        ));
    }

    #[tokio::test]
    async fn test_classify_with_context() {
        let service = FailureClassifierServiceImpl;
        let input = ClassifyFailureInput {
            error_message: "something failed".to_string(),
            operation_context: Some("running test suite".to_string()),
            source: Some("pytest".to_string()),
        };
        let output = service.classify(input).await.unwrap();
        // "fail" + context "test" → TestFailure
        assert_eq!(output.failure_type, FailureType::TestFailure);
    }

    #[tokio::test]
    async fn test_classify_output_has_explanation() {
        let service = FailureClassifierServiceImpl;
        let input = ClassifyFailureInput {
            error_message: "connection timeout".to_string(),
            operation_context: None,
            source: None,
        };
        let output = service.classify(input).await.unwrap();
        assert!(output.explanation.is_some());
        assert!(output.explanation.unwrap().contains("transient"));
    }

    // -----------------------------------------------------------------------
    // classify_type() tests
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_classify_type_transient() {
        let service = FailureClassifierServiceImpl;
        let result = service
            .classify_type("network timeout occurred")
            .await
            .unwrap();
        assert_eq!(result, FailureType::Transient);
    }

    #[tokio::test]
    async fn test_classify_type_test_failure() {
        let service = FailureClassifierServiceImpl;
        let result = service
            .classify_type("test error: assertion failed")
            .await
            .unwrap();
        assert_eq!(result, FailureType::TestFailure);
    }

    #[tokio::test]
    async fn test_classify_type_empty_fails() {
        let service = FailureClassifierServiceImpl;
        let result = service.classify_type("").await;
        assert!(result.is_err());
    }

    // -----------------------------------------------------------------------
    // check_retry_eligibility() tests
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_eligibility_transient_retryable() {
        let service = FailureClassifierServiceImpl;
        let input = CheckRetryEligibilityInput {
            failure_type: FailureType::Transient,
            current_retry_count: Some(0),
            max_retries: Some(3),
        };
        let output = service.check_retry_eligibility(input).await.unwrap();
        assert!(output.eligible);
        assert_eq!(output.remaining_attempts, Some(3));
    }

    #[tokio::test]
    async fn test_eligibility_transient_exhausted() {
        let service = FailureClassifierServiceImpl;
        let input = CheckRetryEligibilityInput {
            failure_type: FailureType::Transient,
            current_retry_count: Some(3),
            max_retries: Some(3),
        };
        let output = service.check_retry_eligibility(input).await.unwrap();
        assert!(!output.eligible);
        assert_eq!(output.remaining_attempts, Some(0));
    }

    #[tokio::test]
    async fn test_eligibility_non_retryable() {
        let service = FailureClassifierServiceImpl;
        let input = CheckRetryEligibilityInput {
            failure_type: FailureType::NonRetryable,
            current_retry_count: None,
            max_retries: None,
        };
        let output = service.check_retry_eligibility(input).await.unwrap();
        assert!(!output.eligible);
        assert_eq!(output.remaining_attempts, None);
    }

    #[tokio::test]
    async fn test_eligibility_default_max_retries() {
        let service = FailureClassifierServiceImpl;
        let input = CheckRetryEligibilityInput {
            failure_type: FailureType::Transient,
            current_retry_count: Some(2),
            max_retries: None, // should default to 3
        };
        let output = service.check_retry_eligibility(input).await.unwrap();
        assert!(output.eligible);
        assert_eq!(output.remaining_attempts, Some(1));
    }

    #[tokio::test]
    async fn test_eligibility_build_failure_not_retryable() {
        let service = FailureClassifierServiceImpl;
        let input = CheckRetryEligibilityInput {
            failure_type: FailureType::BuildFailure,
            current_retry_count: Some(0),
            max_retries: Some(5),
        };
        let output = service.check_retry_eligibility(input).await.unwrap();
        assert!(!output.eligible);
    }

    #[tokio::test]
    async fn test_eligibility_reason_provided() {
        let service = FailureClassifierServiceImpl;
        let input = CheckRetryEligibilityInput {
            failure_type: FailureType::Transient,
            current_retry_count: Some(1),
            max_retries: Some(3),
        };
        let output = service.check_retry_eligibility(input).await.unwrap();
        assert!(!output.reason.is_empty());
    }

    // -----------------------------------------------------------------------
    // default_strategy_for() tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_default_strategy_transient() {
        assert_eq!(
            default_strategy_for(&FailureType::Transient),
            RetryStrategy::SameOperation
        );
    }

    #[test]
    fn test_default_strategy_lsp() {
        assert_eq!(
            default_strategy_for(&FailureType::LspConflict),
            RetryStrategy::ReExecute
        );
    }

    #[test]
    fn test_default_strategy_resource() {
        assert_eq!(
            default_strategy_for(&FailureType::ResourceExhausted),
            RetryStrategy::Fallback
        );
    }

    #[test]
    fn test_default_strategy_system() {
        assert_eq!(
            default_strategy_for(&FailureType::SystemError),
            RetryStrategy::Fallback
        );
    }

    #[test]
    fn test_default_strategy_test_failure() {
        assert!(matches!(
            default_strategy_for(&FailureType::TestFailure),
            RetryStrategy::PatchWithFeedback { .. }
        ));
    }

    #[test]
    fn test_default_strategy_build_failure() {
        assert!(matches!(
            default_strategy_for(&FailureType::BuildFailure),
            RetryStrategy::PatchWithFeedback { .. }
        ));
    }
}
