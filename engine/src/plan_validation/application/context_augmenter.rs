//! ContextAugmenter — transforms failure analysis into augmented planning context.
//!
//! @canonical .pi/architecture/modules/plan-validation.md#augmenter
//! Implements: Contract Freeze — ContextAugmenter, augments UserIntent with failure context
//! Issue: issue-contract-freeze
//!
//! The ContextAugmenter is responsible for transforming structured failure
//! analysis into augmented context that the LLM can use for self-correction.
//! When a template execution fails, the augmenter appends failure details
//! (error locations, suggested fixes, available symbols) to the original
//! user intent, producing a new augmented intent that guides the LLM to
//! fix the specific issues.
//!
//! # Contract (Frozen)
//! - augments UserIntent by appending failure context text
//! - Detects repeated failures (LLM not learning from feedback)
//! - Formats failures in a human-readable LLM-friendly format
//! - No state mutation — returns new UserIntent instances
//! - Parser-agnostic — delegates formatting to FailureParserService

use crate::failure_parser::domain::TemplateFailure;
use crate::planning::domain::intent::UserIntent;

use super::dto::{AugmentIntentInput, AugmentIntentOutput, CheckRepeatedFailureOutput};

/// Transforms failure analysis into augmented planning context.
///
/// Appends structured failure feedback to the user intent so that
/// the LLM can self-correct on retry. Detects repeated failures
/// to flag when the LLM is not learning from feedback.
///
/// # Contract (Frozen)
/// - `augment_intent` — Appends failure context to a UserIntent
/// - `is_repeated_failure` — Checks if a failure repeats a previous one
/// - No implementation logic beyond text formatting and equality checks
/// - All methods are pure functions (no side effects, no state)
pub struct ContextAugmenter;

impl ContextAugmenter {
    /// Augment a user intent with failure analysis for re-planning.
    ///
    /// Produces a new `UserIntent` with the original intent text
    /// followed by structured failure context from the most recent
    /// iteration, plus any additional guidance for repeated failures.
    ///
    /// # Format
    ///
    /// The augmented intent follows this structure:
    ///
    /// ```text
    /// Original intent: <original user intent>
    ///
    /// --- PREVIOUS EXECUTION FAILED ---
    /// <failure 1 details>
    /// <failure 2 details>
    /// ...
    ///
    /// <if repeated failure> This is attempt N. Previous attempts also failed.
    /// Ensure this fix addresses ALL previously reported errors.
    ///
    /// Generate corrected content. Do NOT repeat the same mistakes.
    /// ```
    ///
    /// # Contract
    ///
    /// - Creates a new `UserIntent` (original is not mutated)
    /// - Appends failure details in a structured format
    /// - Detects and flags repeated failures
    /// - Returns the augmented intent and metadata about the augmentation
    pub fn augment_intent(input: AugmentIntentInput) -> AugmentIntentOutput {
        let mut augmented_text = format!("Original intent: {}\n\n", input.intent.input);

        augmented_text.push_str("--- PREVIOUS EXECUTION FAILED ---\n");

        for (idx, failure) in input.failures.iter().enumerate() {
            augmented_text.push_str(&format!("  {}. ", idx + 1));
            augmented_text.push_str(&Self::format_failure(failure));
            augmented_text.push('\n');
        }

        if !input.failure_history.is_empty() {
            augmented_text.push_str(&format!(
                "\nThis is attempt {}/{}. Previous attempts also failed. \
                 Ensure this fix addresses ALL previously reported errors.",
                input.iteration, input.max_iterations
            ));
        }

        augmented_text.push_str("\n\nGenerate corrected content. Do NOT repeat the same mistakes.");

        let has_repeated = Self::check_repeated_failures(&input.failures, &input.failure_history);

        let unique_types = Self::count_unique_failure_types(&input.failures);

        // Preserve the original execution_id and session_id from the intent
        let augmented_intent = UserIntent::new(augmented_text, input.intent.execution_id);

        AugmentIntentOutput {
            augmented_intent,
            has_repeated_failures: has_repeated.is_repeated,
            unique_failure_types: unique_types,
        }
    }

    /// Check if a failure repeats a failure from a previous iteration.
    ///
    /// Repeated failures suggest the LLM didn't understand the fix or
    /// the fix introduced the same error pattern. This is a signal to
    /// escalate or change the augmentation strategy.
    ///
    /// # Contract
    ///
    /// - Exact equality check on `TemplateFailure` variants
    /// - Returns the first iteration where the failure was seen
    /// - Returns `is_repeated: false` with empty history
    pub fn check_repeated_failures(
        current_failures: &[TemplateFailure],
        failure_history: &[Vec<TemplateFailure>],
    ) -> CheckRepeatedFailureOutput {
        let mut first_seen: Option<u32> = None;
        let mut repeat_count = 0u32;

        for current in current_failures {
            for (iter_idx, prev_iter) in failure_history.iter().enumerate() {
                if prev_iter.contains(current) {
                    let iter_num = (iter_idx + 1) as u32;
                    if first_seen.is_none() {
                        first_seen = Some(iter_num);
                    }
                    repeat_count += 1;
                }
            }
        }

        CheckRepeatedFailureOutput {
            is_repeated: repeat_count > 0,
            first_seen_iteration: first_seen,
            repeat_count,
        }
    }

    /// Format a single TemplateFailure into a human-readable string.
    fn format_failure(failure: &TemplateFailure) -> String {
        match failure {
            TemplateFailure::MissingSymbol {
                symbol,
                available,
                suggestion,
                location,
            } => {
                let loc_str = match location.column {
                    Some(col) => format!("{}:{}:{}", location.file, location.line, col),
                    None => format!("{}:{}", location.file, location.line),
                };
                let mut msg = format!("Missing symbol '{}' at {}", symbol, loc_str);
                if let Some(sugg) = suggestion {
                    msg.push_str(&format!(". Suggested fix: {}", sugg));
                }
                if !available.is_empty() {
                    msg.push_str(&format!(". Available symbols: {}", available.join(", ")));
                }
                msg
            }
            TemplateFailure::WrongArgCount {
                function,
                expected,
                actual,
                location,
            } => {
                format!(
                    "Wrong argument count for '{}' at {}:{}. Expected {}, got {}",
                    function, location.file, location.line, expected, actual
                )
            }
            TemplateFailure::TypeMismatch {
                expected,
                actual,
                location,
            } => {
                let loc_str = match location.column {
                    Some(col) => format!("{}:{}:{}", location.file, location.line, col),
                    None => format!("{}:{}", location.file, location.line),
                };
                format!(
                    "Type mismatch at {}. Expected '{}', got '{}'",
                    loc_str, expected, actual
                )
            }
            TemplateFailure::CompileError {
                code,
                message,
                location,
            } => {
                format!(
                    "Compile error [{}] at {}:{}: {}",
                    code, location.file, location.line, message
                )
            }
            TemplateFailure::AssertionFailure {
                test_name,
                expected,
                received,
                location,
            } => {
                format!(
                    "Assertion failure in '{}' at {}:{}: expected '{}', received '{}'",
                    test_name, location.file, location.line, expected, received
                )
            }
            TemplateFailure::TestFailure {
                test_name,
                message,
                location,
            } => {
                let loc_str = match location {
                    Some(loc) => format!(" at {}:{}", loc.file, loc.line),
                    None => String::new(),
                };
                format!("Test failure '{}'{}: {}", test_name, loc_str, message)
            }
        }
    }

    /// Count the number of unique failure types in a list.
    fn count_unique_failure_types(failures: &[TemplateFailure]) -> u32 {
        let mut types = Vec::new();
        for f in failures {
            let type_name = match f {
                TemplateFailure::MissingSymbol { .. } => "missing_symbol",
                TemplateFailure::WrongArgCount { .. } => "wrong_arg_count",
                TemplateFailure::TypeMismatch { .. } => "type_mismatch",
                TemplateFailure::CompileError { .. } => "compile_error",
                TemplateFailure::AssertionFailure { .. } => "assertion_failure",
                TemplateFailure::TestFailure { .. } => "test_failure",
            };
            if !types.contains(&type_name) {
                types.push(type_name);
            }
        }
        types.len() as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::failure_parser::domain::failure::SourceLocation;

    fn sample_missing_symbol() -> TemplateFailure {
        TemplateFailure::MissingSymbol {
            symbol: "addTask".into(),
            available: vec!["add".into(), "remove".into()],
            suggestion: Some("use 'add' instead of 'addTask'".into()),
            location: SourceLocation::new("test.ts", 10, None),
        }
    }

    fn sample_compile_error() -> TemplateFailure {
        TemplateFailure::CompileError {
            code: "TS2339".into(),
            message: "Property 'foo' does not exist on type 'Bar'".into(),
            location: SourceLocation::new("src/bar.ts", 42, None),
        }
    }

    fn sample_assertion_failure() -> TemplateFailure {
        TemplateFailure::AssertionFailure {
            test_name: "should_add_task".into(),
            expected: "3".into(),
            received: "2".into(),
            location: SourceLocation::new("tests/tasklist.test.ts", 25, None),
        }
    }

    fn make_intent(text: &str) -> UserIntent {
        UserIntent::new(text.to_string(), None)
    }

    #[test]
    fn test_augment_intent_first_failure() {
        let intent = make_intent("Add a getActiveTasks method to TaskList");
        let failures = vec![sample_missing_symbol()];

        let output = ContextAugmenter::augment_intent(AugmentIntentInput {
            intent,
            failures,
            failure_history: vec![],
            iteration: 2,
            max_iterations: 3,
        });

        assert!(output.augmented_intent.input.contains("Original intent:"));
        assert!(
            output
                .augmented_intent
                .input
                .contains("PREVIOUS EXECUTION FAILED")
        );
        assert!(
            output
                .augmented_intent
                .input
                .contains("Missing symbol 'addTask'")
        );
        assert!(output.augmented_intent.input.contains("test.ts:10"));
        assert!(!output.has_repeated_failures);
        assert_eq!(output.unique_failure_types, 1);
    }

    #[test]
    fn test_augment_intent_with_history() {
        let intent = make_intent("Add a new method");
        let failures = vec![sample_compile_error()];

        let output = ContextAugmenter::augment_intent(AugmentIntentInput {
            intent,
            failures,
            failure_history: vec![vec![sample_missing_symbol()]],
            iteration: 2,
            max_iterations: 3,
        });

        assert!(output.augmented_intent.input.contains("attempt 2/3"));
    }

    #[test]
    fn test_augment_intent_multiple_failures() {
        let intent = make_intent("Implement feature");
        let failures = vec![
            sample_missing_symbol(),
            sample_compile_error(),
            sample_assertion_failure(),
        ];

        let output = ContextAugmenter::augment_intent(AugmentIntentInput {
            intent,
            failures,
            failure_history: vec![],
            iteration: 2,
            max_iterations: 3,
        });

        assert!(output.augmented_intent.input.contains("1. "));
        assert!(output.augmented_intent.input.contains("2. "));
        assert!(output.augmented_intent.input.contains("3. "));
        assert_eq!(output.unique_failure_types, 3);
    }

    #[test]
    fn test_augment_intent_includes_available_symbols() {
        let intent = make_intent("Use addTask");
        let failures = vec![sample_missing_symbol()];

        let output = ContextAugmenter::augment_intent(AugmentIntentInput {
            intent,
            failures,
            failure_history: vec![],
            iteration: 2,
            max_iterations: 3,
        });

        assert!(
            output
                .augmented_intent
                .input
                .contains("Available symbols: add, remove")
        );
        assert!(
            output
                .augmented_intent
                .input
                .contains("Suggested fix: use 'add' instead of 'addTask'")
        );
    }

    #[test]
    fn test_augment_intent_includes_suggestion() {
        let intent = make_intent("Fix tests");
        let failures = vec![sample_assertion_failure()];

        let output = ContextAugmenter::augment_intent(AugmentIntentInput {
            intent,
            failures,
            failure_history: vec![],
            iteration: 2,
            max_iterations: 3,
        });

        assert!(
            output
                .augmented_intent
                .input
                .contains("Assertion failure in 'should_add_task'")
        );
        assert!(
            output
                .augmented_intent
                .input
                .contains("expected '3', received '2'")
        );
    }

    #[test]
    fn test_augment_intent_empty_failures() {
        let intent = make_intent("Do something");
        let failures: Vec<TemplateFailure> = vec![];

        let output = ContextAugmenter::augment_intent(AugmentIntentInput {
            intent,
            failures,
            failure_history: vec![],
            iteration: 1,
            max_iterations: 3,
        });

        assert!(
            output
                .augmented_intent
                .input
                .contains("PREVIOUS EXECUTION FAILED")
        );
        assert_eq!(output.unique_failure_types, 0);
    }

    #[test]
    fn test_check_repeated_failures_no_history() {
        let result = ContextAugmenter::check_repeated_failures(&[sample_missing_symbol()], &[]);
        assert!(!result.is_repeated);
        assert!(result.first_seen_iteration.is_none());
        assert_eq!(result.repeat_count, 0);
    }

    #[test]
    fn test_check_repeated_failures_detected() {
        let result = ContextAugmenter::check_repeated_failures(
            &[sample_missing_symbol()],
            &[vec![sample_missing_symbol()]],
        );
        assert!(result.is_repeated);
        assert_eq!(result.first_seen_iteration, Some(1));
        assert_eq!(result.repeat_count, 1);
    }

    #[test]
    fn test_check_repeated_failures_partial_match() {
        let result = ContextAugmenter::check_repeated_failures(
            &[sample_missing_symbol(), sample_compile_error()],
            &[vec![sample_missing_symbol()]],
        );
        assert!(result.is_repeated);
        assert_eq!(result.repeat_count, 1);
    }

    #[test]
    fn test_check_repeated_failures_no_match() {
        let result = ContextAugmenter::check_repeated_failures(
            &[sample_compile_error()],
            &[vec![sample_missing_symbol()]],
        );
        assert!(!result.is_repeated);
        assert_eq!(result.repeat_count, 0);
    }

    #[test]
    fn test_format_wrong_arg_count() {
        let failure = TemplateFailure::WrongArgCount {
            function: "calculate".into(),
            expected: 2,
            actual: 3,
            location: SourceLocation::new("math.ts", 15, None),
        };

        let output = ContextAugmenter::augment_intent(AugmentIntentInput {
            intent: make_intent("test"),
            failures: vec![failure],
            failure_history: vec![],
            iteration: 1,
            max_iterations: 3,
        });

        assert!(
            output
                .augmented_intent
                .input
                .contains("Wrong argument count for 'calculate'")
        );
        assert!(output.augmented_intent.input.contains("Expected 2, got 3"));
    }

    #[test]
    fn test_format_type_mismatch() {
        let failure = TemplateFailure::TypeMismatch {
            expected: "string".into(),
            actual: "number".into(),
            location: SourceLocation::new("types.ts", 5, Some(10)),
        };

        let output = ContextAugmenter::augment_intent(AugmentIntentInput {
            intent: make_intent("test"),
            failures: vec![failure],
            failure_history: vec![],
            iteration: 1,
            max_iterations: 3,
        });

        assert!(output.augmented_intent.input.contains("Type mismatch"));
        assert!(
            output
                .augmented_intent
                .input
                .contains("Expected 'string', got 'number'")
        );
        assert!(output.augmented_intent.input.contains("types.ts:5:10"));
    }
}
