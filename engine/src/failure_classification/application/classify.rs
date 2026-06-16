//! Standalone `classify_failure()` function — simple error message classification.
//!
//! @canonical .pi/architecture/modules/failure-classification.md#classifier
//! Implements: classify_failure() — free function for quick classification
//! Issue: #36
//!
//! Provides a standalone `classify_failure()` function that classifies an error
//! message into a `FailureType` using the same pattern matching logic as the
//! full `FailureClassifierService`. This is the simplest entry point for
//! callers who just need `error_message → FailureType`.
//!
//! # Usage
//!
//! ```ignore
//! use rigorix::failure_classification::application::classify::classify_failure;
//!
//! let failure_type = classify_failure("connection timeout");
//! assert_eq!(failure_type, FailureType::Transient);
//! ```

use crate::failure_classification::domain::FailureType;

/// Classify an error message into a `FailureType`.
///
/// This is the simplest entry point for failure classification. It performs
/// case-insensitive substring matching against known error patterns and
/// returns the appropriate `FailureType`.
///
/// Returns `FailureType::NonRetryable` if no pattern matches or the message
/// is empty.
///
/// # Pattern Matching Rules (in priority order)
///
/// 1. **Resource Exhaustion**: "out of memory", "oom", "disk full", "no space"
/// 2. **System Error**: "signal", "process crash", "killed", "segfault", "io error"
/// 3. **Build Failure**: "build fail", "compile error", "compilation error"
/// 4. **Test Failure**: "test fail", "test error", "tests failed"
/// 5. **LSP Conflict**: "lsp", "type error", "type mismatch", "type conflict"
/// 6. **Transient**: "timeout", "connection", "network", "rate limit", "429"
/// 7. **NonRetryable**: default fallback
///
/// # Examples
///
/// ```
/// use rigorix::failure_classification::application::classify::classify_failure;
/// use rigorix::failure_classification::domain::FailureType;
///
/// assert_eq!(classify_failure("connection timed out"), FailureType::Transient);
/// assert_eq!(classify_failure("tests failed with errors"), FailureType::TestFailure);
/// assert_eq!(classify_failure("invalid API key"), FailureType::NonRetryable);
/// ```
pub fn classify_failure(error_message: &str) -> FailureType {
    let msg = error_message.trim().to_lowercase();
    if msg.is_empty() {
        return FailureType::NonRetryable;
    }

    // Resource exhaustion (most critical — check first)
    if contains_any(&msg, &["out of memory", "oom", "disk full", "no space"]) {
        return FailureType::ResourceExhausted;
    }

    // System errors
    if contains_any(
        &msg,
        &[
            "signal",
            "process crash",
            "killed",
            "segmentation fault",
            "segfault",
            "core dump",
            "io error",
            "broken pipe",
        ],
    ) {
        return FailureType::SystemError;
    }

    // Build failures
    if contains_any(
        &msg,
        &[
            "build fail",
            "compile error",
            "compilation error",
            "build error",
            "build failed",
        ],
    ) {
        return FailureType::BuildFailure;
    }

    // Test failures — architecture spec: "test" + "fail"/"error"
    // Both keywords must appear in the message (not necessarily adjacent)
    if contains_any(&msg, &["test"]) && contains_any(&msg, &["fail", "error"]) {
        return FailureType::TestFailure;
    }

    // LSP conflicts
    if contains_any(
        &msg,
        &[
            "lsp",
            "type error",
            "type mismatch",
            "type conflict",
            "cannot find type",
            "does not implement",
        ],
    ) {
        return FailureType::LspConflict;
    }

    // Transient errors
    if contains_any(
        &msg,
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
        ],
    ) {
        return FailureType::Transient;
    }

    // Default
    FailureType::NonRetryable
}

/// Returns `true` if `text` contains any of the given substrings.
fn contains_any(text: &str, patterns: &[&str]) -> bool {
    patterns.iter().any(|p| text.contains(p))
}

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // Transient
    // -----------------------------------------------------------------------

    #[test]
    fn test_classify_timeout() {
        assert_eq!(classify_failure("timeout"), FailureType::Transient);
    }

    #[test]
    fn test_classify_timed_out() {
        assert_eq!(classify_failure("timed out"), FailureType::Transient);
    }

    #[test]
    fn test_classify_connection_refused() {
        assert_eq!(
            classify_failure("connection refused"),
            FailureType::Transient
        );
    }

    #[test]
    fn test_classify_network_unreachable() {
        assert_eq!(
            classify_failure("network unreachable"),
            FailureType::Transient
        );
    }

    #[test]
    fn test_classify_rate_limit() {
        assert_eq!(
            classify_failure("rate limit exceeded"),
            FailureType::Transient
        );
    }

    #[test]
    fn test_classify_http_429() {
        assert_eq!(
            classify_failure("HTTP 429: Too Many Requests"),
            FailureType::Transient
        );
    }

    #[test]
    fn test_classify_http_503() {
        assert_eq!(
            classify_failure("503 Service Unavailable"),
            FailureType::Transient
        );
    }

    // -----------------------------------------------------------------------
    // Test Failure
    // -----------------------------------------------------------------------

    #[test]
    fn test_classify_test_failed() {
        assert_eq!(
            classify_failure("tests failed with 3 errors"),
            FailureType::TestFailure
        );
    }

    #[test]
    fn test_classify_test_error_assertion() {
        assert_eq!(
            classify_failure("test error: assertion failed at main.rs:42"),
            FailureType::TestFailure
        );
    }

    #[test]
    fn test_classify_test_fail_basic() {
        assert_eq!(
            classify_failure("test 'unit_test_1' FAILED"),
            FailureType::TestFailure
        );
    }

    // -----------------------------------------------------------------------
    // Build Failure
    // -----------------------------------------------------------------------

    #[test]
    fn test_classify_build_fail() {
        assert_eq!(classify_failure("build failed"), FailureType::BuildFailure);
    }

    #[test]
    fn test_classify_compile_error() {
        assert_eq!(
            classify_failure("compile error: undefined reference to 'main'"),
            FailureType::BuildFailure
        );
    }

    #[test]
    fn test_classify_build_error() {
        assert_eq!(
            classify_failure("build error: cannot compile package"),
            FailureType::BuildFailure
        );
    }

    #[test]
    fn test_classify_compilation_error() {
        assert_eq!(
            classify_failure("compilation error in src/lib.rs"),
            FailureType::BuildFailure
        );
    }

    // -----------------------------------------------------------------------
    // LSP Conflict
    // -----------------------------------------------------------------------

    #[test]
    fn test_classify_lsp_generic() {
        assert_eq!(classify_failure("LSP error"), FailureType::LspConflict);
    }

    #[test]
    fn test_classify_type_mismatch() {
        assert_eq!(
            classify_failure("type mismatch: expected String, found i32"),
            FailureType::LspConflict
        );
    }

    #[test]
    fn test_classify_type_error() {
        assert_eq!(
            classify_failure("type error[E0308]"),
            FailureType::LspConflict
        );
    }

    #[test]
    fn test_classify_cannot_find_type() {
        assert_eq!(
            classify_failure("cannot find type `Foo` in this scope"),
            FailureType::LspConflict
        );
    }

    // -----------------------------------------------------------------------
    // Resource Exhausted
    // -----------------------------------------------------------------------

    #[test]
    fn test_classify_oom() {
        assert_eq!(
            classify_failure("out of memory: cannot allocate 1GB"),
            FailureType::ResourceExhausted
        );
    }

    #[test]
    fn test_classify_disk_full() {
        assert_eq!(
            classify_failure("disk full: no space left on device"),
            FailureType::ResourceExhausted
        );
    }

    #[test]
    fn test_classify_oom_abbreviation() {
        assert_eq!(
            classify_failure("OOM killer terminated process"),
            FailureType::ResourceExhausted
        );
    }

    // -----------------------------------------------------------------------
    // System Error
    // -----------------------------------------------------------------------

    #[test]
    fn test_classify_signal() {
        assert_eq!(classify_failure("signal 9"), FailureType::SystemError);
    }

    #[test]
    fn test_classify_killed() {
        assert_eq!(classify_failure("process killed"), FailureType::SystemError);
    }

    #[test]
    fn test_classify_segfault() {
        assert_eq!(
            classify_failure("segmentation fault (core dumped)"),
            FailureType::SystemError
        );
    }

    #[test]
    fn test_classify_io_error() {
        assert_eq!(
            classify_failure("IO error: No such file or directory"),
            FailureType::SystemError
        );
    }

    // -----------------------------------------------------------------------
    // Non-Retryable (default)
    // -----------------------------------------------------------------------

    #[test]
    fn test_classify_invalid_api_key() {
        assert_eq!(
            classify_failure("invalid api key"),
            FailureType::NonRetryable
        );
    }

    #[test]
    fn test_classify_auth_error() {
        assert_eq!(
            classify_failure("authentication failed: unauthorized"),
            FailureType::NonRetryable
        );
    }

    #[test]
    fn test_classify_unknown_error() {
        assert_eq!(
            classify_failure("something completely unexpected"),
            FailureType::NonRetryable
        );
    }

    #[test]
    fn test_classify_empty_message() {
        assert_eq!(classify_failure(""), FailureType::NonRetryable);
    }

    #[test]
    fn test_classify_whitespace() {
        assert_eq!(classify_failure("   "), FailureType::NonRetryable);
    }

    // -----------------------------------------------------------------------
    // Case Insensitivity
    // -----------------------------------------------------------------------

    #[test]
    fn test_case_insensitive_build() {
        assert_eq!(classify_failure("BUILD FAILED"), FailureType::BuildFailure);
    }

    #[test]
    fn test_case_insensitive_lsp() {
        assert_eq!(classify_failure("LSP"), FailureType::LspConflict);
    }

    #[test]
    fn test_case_insensitive_timeout() {
        assert_eq!(classify_failure("TIMEOUT"), FailureType::Transient);
    }

    // -----------------------------------------------------------------------
    // Priority: more specific patterns matched first
    // -----------------------------------------------------------------------

    #[test]
    fn test_priority_resource_over_system() {
        // "out of memory" should match ResourceExhausted, not SystemError
        assert_eq!(
            classify_failure("process killed: out of memory"),
            FailureType::ResourceExhausted
        );
    }

    #[test]
    fn test_priority_system_over_build() {
        // "build signal" should match SystemError (signal), not BuildFailure
        // Actually this contains "signal" which matches SystemError first
        assert_eq!(
            classify_failure("build received signal 9"),
            FailureType::SystemError
        );
    }

    #[test]
    fn test_priority_build_over_lsp() {
        // "compile error" should match BuildFailure, not LspConflict
        assert_eq!(
            classify_failure("compile error: type mismatch"),
            FailureType::BuildFailure
        );
    }

    // -----------------------------------------------------------------------
    // Edge Cases
    // -----------------------------------------------------------------------

    #[test]
    fn test_classify_substring_in_word() {
        // "signal" is part of many words - ensure it still matches
        assert_eq!(
            classify_failure("process signaled termination"),
            FailureType::SystemError
        );
    }

    #[test]
    fn test_classify_very_long_message() {
        let long_msg = "a".repeat(10_000) + "timeout" + &"b".repeat(10_000);
        assert_eq!(classify_failure(&long_msg), FailureType::Transient);
    }

    #[test]
    fn test_classify_special_characters() {
        assert_eq!(
            classify_failure("connection timeout! (code: 0xDEAD)"),
            FailureType::Transient
        );
    }

    #[test]
    fn test_classify_mixed_case_lsp() {
        assert_eq!(
            classify_failure("LSP Type Mismatch: Expected Int32, got String"),
            FailureType::LspConflict
        );
    }

    // -----------------------------------------------------------------------
    // Consistency: same results as service implementation
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_consistency_with_service() {
        use crate::failure_classification::application::{
            FailureClassifierService, FailureClassifierServiceImpl,
        };
        use crate::failure_classification::domain::FailureType;

        let service = FailureClassifierServiceImpl;
        let test_cases = vec![
            ("connection timeout", FailureType::Transient),
            ("tests failed", FailureType::TestFailure),
            ("build error", FailureType::BuildFailure),
            ("LSP type mismatch", FailureType::LspConflict),
            ("out of memory", FailureType::ResourceExhausted),
            ("process killed", FailureType::SystemError),
            ("invalid input", FailureType::NonRetryable),
        ];

        for (msg, expected) in test_cases {
            let free_result = classify_failure(msg);
            let service_result = service
                .classify_type(msg)
                .await
                .unwrap_or(FailureType::NonRetryable);
            assert_eq!(
                free_result, expected,
                "Free function mismatch for '{}': expected {:?}, got {:?}",
                msg, expected, free_result
            );
            assert_eq!(
                service_result, expected,
                "Service mismatch for '{}': expected {:?}, got {:?}",
                msg, expected, service_result
            );
        }
    }
}
