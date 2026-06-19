//! Implementation of `FixSuggestionService`.
//!
//! @canonical .pi/architecture/modules/failure-parser.md#suggested-fix-generation
//! Implements: FixSuggestionService — generate suggested fixes from failures and source context
//! Issue: #499
//!
//! Suggests fixes by cross-referencing TemplateFailure against available source context
//! symbols. Uses multiple strategies: substring matching, case-insensitive matching,
//! character-level similarity, source symbol analysis, and type annotation checks.

use async_trait::async_trait;
use std::collections::HashMap;

use crate::failure_parser::domain::{
    failure::{SourceLocation, TemplateFailure},
    FailureParserError, SourceContext,
};

use super::service::FixSuggestionService;

/// Implementation of `FixSuggestionService`.
///
/// Generates fix suggestions by analyzing:
/// - Exact and substring symbol matches against source context
/// - Character-level similarity (first-letter, prefix matching)
/// - Available symbol lists from the compiler
/// - Type annotations in source context
pub struct FixSuggestionServiceImpl;

impl FixSuggestionServiceImpl {
    /// Create a new FixSuggestionServiceImpl.
    pub fn new() -> Self {
        Self
    }

    /// Core suggestion generation logic.
    ///
    /// Analyzes a failure and source context to produce an actionable suggestion.
    fn generate_suggestion(
        failure: &TemplateFailure,
        source_context: &SourceContext,
    ) -> Option<String> {
        match failure {
            TemplateFailure::MissingSymbol {
                symbol,
                available,
                location,
                ..
            } => Self::suggest_missing_symbol(symbol, available, location, source_context),

            TemplateFailure::WrongArgCount {
                function,
                expected,
                actual,
                ..
            } => {
                if *expected > *actual {
                    Some(format!(
                        "'{function}' expects {expected} arguments but {actual} were provided. \
                         You are missing {missing} argument{s}. \
                         Check the function signature and add the required parameter{s}.",
                        function = function,
                        expected = expected,
                        actual = actual,
                        missing = expected - actual,
                        s = if *expected - *actual == 1 { "" } else { "s" },
                    ))
                } else {
                    Some(format!(
                        "'{function}' expects {expected} arguments but {actual} were provided. \
                         Remove {extra} extra argument{s}.",
                        function = function,
                        expected = expected,
                        actual = actual,
                        extra = actual - expected,
                        s = if *actual - *expected == 1 { "" } else { "s" },
                    ))
                }
            }

            TemplateFailure::TypeMismatch {
                expected,
                actual,
                location,
            } => Self::suggest_type_mismatch(expected, actual, location, source_context),

            TemplateFailure::CompileError { code, message, .. } => {
                Self::suggest_compile_error(code, message)
            }

            TemplateFailure::AssertionFailure {
                test_name,
                expected,
                received,
                ..
            } => {
                if expected.len() <= 80 && received.len() <= 80 {
                    Some(format!(
                        "Test '{test_name}' failed: expected '{expected}' but received '{received}'.\n\
                         - Check the assertion logic — are you comparing the correct values?\n\
                         - Verify the function under test returns '{expected}' for this input.\n\
                         - Debug by logging the intermediate values before the assertion.",
                        test_name = test_name,
                        expected = expected,
                        received = received,
                    ))
                } else {
                    Some(format!(
                        "Test '{test_name}' failed: expected value does not match received value.\n\
                         - Check the assertion logic and expected values.\n\
                         - Verify the function under test returns the correct value for this input.",
                        test_name = test_name,
                    ))
                }
            }

            TemplateFailure::TestFailure {
                test_name, message, ..
            } => {
                let msg_snippet = if message.len() > 100 {
                    format!("{}...", &message[..100])
                } else {
                    message.clone()
                };
                Some(format!(
                    "Test '{test_name}' failed with: {msg_snippet}\n\
                     - Check if the test environment is properly set up.\n\
                     - Verify that all test dependencies are available.\n\
                     - If this is a timeout, consider increasing the test timeout or optimizing the test.",
                    test_name = test_name,
                    msg_snippet = msg_snippet,
                ))
            }
        }
    }

    /// Generate suggestion for MissingSymbol failures.
    fn suggest_missing_symbol(
        symbol: &str,
        available: &[String],
        location: &SourceLocation,
        source_context: &SourceContext,
    ) -> Option<String> {
        // Strategy 1: Check if symbol is in the "available" list from compiler
        if let Some(matched) = Self::find_best_match_in_list(symbol, available) {
            return Some(format!(
                "Use '{matched}' instead of '{symbol}' (similar name in scope)."
            ));
        }

        // Strategy 2: Check source context symbols from the same file
        let file_symbols = source_context.symbols_in_file(&location.file);
        if let Some(matched) = Self::find_best_match_in_list(symbol, &file_symbols) {
            return Some(format!(
                "Use '{matched}' instead of '{symbol}' (available in this file)."
            ));
        }

        // Strategy 3: Check all symbols from all files (broader search)
        let all_symbols: Vec<String> = source_context
            .symbols_by_file
            .values()
            .flat_map(|v| v.iter().cloned())
            .collect();

        if let Some(matched) = Self::find_best_match_in_list(symbol, &all_symbols) {
            return Some(format!(
                "Use '{matched}' instead of '{symbol}' (found in project source)."
            ));
        }

        // Strategy 4: If no match but we have available symbols, list them
        if !available.is_empty() {
            let listed = available
                .iter()
                .map(|s| format!("'{}'", s))
                .collect::<Vec<_>>()
                .join(", ");
            return Some(format!(
                "Symbol '{symbol}' not found. Available symbols in scope: {listed}. \
                 Consider using one of these instead.",
                symbol = symbol,
                listed = listed,
            ));
        }

        // No suggestions
        None
    }

    /// Find the best matching symbol in a list, using multiple similarity strategies.
    fn find_best_match_in_list<'a>(symbol: &str, candidates: &'a [String]) -> Option<&'a str> {
        if candidates.is_empty() {
            return None;
        }

        let symbol_lower = symbol.to_lowercase();

        // Strategy A: Exact substring match (one contains the other)
        for candidate in candidates {
            if symbol.contains(candidate.as_str()) || candidate.contains(symbol) {
                return Some(candidate.as_str());
            }
        }

        // Strategy B: Case-insensitive substring match
        for candidate in candidates {
            let cand_lower = candidate.to_lowercase();
            if symbol_lower.contains(&cand_lower) || cand_lower.contains(&symbol_lower) {
                return Some(candidate.as_str());
            }
        }

        // Strategy C: First-letter match (common in TypeScript errors)
        for candidate in candidates {
            if let Some(cand_first) = candidate.chars().next() {
                if let Some(sym_first) = symbol.chars().next() {
                    if cand_first.to_ascii_lowercase() == sym_first.to_ascii_lowercase() {
                        return Some(candidate.as_str());
                    }
                }
            }
        }

        // Strategy D: Share at least 3 characters in the same order
        if symbol.len() >= 3 {
            for candidate in candidates {
                if candidate.len() >= 3 {
                    let sym_chars: Vec<char> = symbol.chars().collect();
                    let cand_chars: Vec<char> = candidate.chars().collect();
                    let shared = sym_chars
                        .iter()
                        .zip(cand_chars.iter())
                        .take_while(|(a, b)| a == b)
                        .count();
                    if shared >= 3 {
                        return Some(candidate.as_str());
                    }
                }
            }
        }

        // No good match found
        None
    }

    /// Generate suggestion for TypeMismatch failures.
    fn suggest_type_mismatch(
        expected: &str,
        actual: &str,
        location: &SourceLocation,
        source_context: &SourceContext,
    ) -> Option<String> {
        // Check if the expected type exists in source context
        let file_symbols = source_context.symbols_in_file(&location.file);
        let type_available = file_symbols.iter().any(|s| {
            s == expected || s.to_lowercase() == expected.to_lowercase()
        });

        let type_hint = if type_available {
            format!(" Type '{}' is defined in this file.", expected)
        } else {
            // Check all files
            let all_types: Vec<&str> = source_context
                .symbols_by_file
                .values()
                .flat_map(|v| v.iter())
                .map(|s| s.as_str())
                .collect();
            if all_types.contains(&expected) {
                format!(" Type '{}' exists elsewhere in the project.", expected)
            } else {
                String::new()
            }
        };

        Some(format!(
            "Type mismatch: expected '{expected}' but got '{actual}'.{type_hint}\n\
             Suggestions:\n\
             - Add a type cast: `value as {expected}` or `{expected}(value)`\n\
             - Change the variable type annotation to match the actual type\n\
             - Check the function's return type — it should be '{expected}'",
            expected = expected,
            actual = actual,
            type_hint = type_hint,
        ))
    }

    /// Generate suggestion for CompileError failures.
    fn suggest_compile_error(code: &str, message: &str) -> Option<String> {
        // Specific guidance for common error codes
        let specific_help = match code {
            "TS1005" => Some("Missing semicolon, comma, or bracket — check for syntax errors near this line."),
            "TS1109" => Some("Expression expected — check for missing operands or incomplete statements."),
            "TS2300" => Some("Duplicate identifier — check for name collisions in imports or declarations."),
            "TS2307" => Some("Cannot find module — verify the import path or install the missing dependency."),
            "TS2580" => Some("Cannot find name — ensure the variable/type is defined before use."),
            "TS7006" => Some("Parameter implicitly has 'any' type — add a type annotation to the function parameter."),
            "TS7031" => Some("Binding element implicitly has 'any' type — add a type annotation to the destructured parameter."),
            _ => None,
        };

        match specific_help {
            Some(help) => Some(format!(
                "{code}: {message}\n{help}",
                code = code,
                message = message,
                help = help,
            )),
            None => Some(format!(
                "{code}: {message}\n\
                 Review the code at the reported location and fix the syntax or type error.",
                code = code,
                message = message,
            )),
        }
    }
}

impl Default for FixSuggestionServiceImpl {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl FixSuggestionService for FixSuggestionServiceImpl {
    async fn suggest_fix(
        &self,
        failure: &TemplateFailure,
        source_context: &SourceContext,
    ) -> Result<Option<String>, FailureParserError> {
        Ok(Self::generate_suggestion(failure, source_context))
    }

    async fn suggest_fixes_batch(
        &self,
        failures: &[TemplateFailure],
        source_context: &SourceContext,
    ) -> Result<Vec<(usize, Option<String>)>, FailureParserError> {
        // Build a cache of file → symbols for efficient lookup
        let _symbol_cache: HashMap<&str, &[String]> = source_context
            .symbols_by_file
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_slice()))
            .collect();

        let results: Vec<(usize, Option<String>)> = failures
            .iter()
            .enumerate()
            .map(|(i, f)| {
                let suggestion = Self::generate_suggestion(f, source_context);
                (i, suggestion)
            })
            .collect();

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx_with_symbols(file: &str, symbols: &[&str]) -> SourceContext {
        let mut ctx = SourceContext::empty();
        ctx.symbols_by_file.insert(
            file.to_string(),
            symbols.iter().map(|s| s.to_string()).collect(),
        );
        ctx
    }

    // -----------------------------------------------------------------------
    // MissingSymbol Suggestions
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_missing_symbol_exact_substring_match() {
        let ctx = ctx_with_symbols("test.ts", &["add", "remove"]);
        let failure = TemplateFailure::MissingSymbol {
            symbol: "addTask".into(),
            available: vec!["add".into()],
            suggestion: None,
            location: SourceLocation::new("test.ts", 3, Some(10)),
        };
        let result = FixSuggestionServiceImpl
            .suggest_fix(&failure, &ctx)
            .await
            .unwrap();
        assert!(result.is_some());
        assert!(result.unwrap().contains("add"));
    }

    #[tokio::test]
    async fn test_missing_symbol_source_context_fallback() {
        let ctx = ctx_with_symbols("test.ts", &["add", "remove", "list"]);
        let failure = TemplateFailure::MissingSymbol {
            symbol: "addItem".into(),
            available: vec![],
            suggestion: None,
            location: SourceLocation::new("test.ts", 3, Some(10)),
        };
        let result = FixSuggestionServiceImpl
            .suggest_fix(&failure, &ctx)
            .await
            .unwrap();
        let sug = result.unwrap();
        assert!(sug.contains("add") || sug.contains("list"));
    }

    #[tokio::test]
    async fn test_missing_symbol_no_match_fallback() {
        let ctx = ctx_with_symbols("test.ts", &["abc", "def"]);
        let failure = TemplateFailure::MissingSymbol {
            symbol: "xyz".into(),
            available: vec!["abc".into(), "def".into()],
            suggestion: None,
            location: SourceLocation::new("test.ts", 3, Some(10)),
        };
        let result = FixSuggestionServiceImpl
            .suggest_fix(&failure, &ctx)
            .await
            .unwrap();
        assert!(result.is_some());
        // Should list available symbols
        let sug = result.unwrap();
        assert!(sug.contains("Available symbols"));
    }

    #[tokio::test]
    async fn test_missing_symbol_no_symbols_at_all() {
        let ctx = ctx_with_symbols("test.ts", &[]);
        let failure = TemplateFailure::MissingSymbol {
            symbol: "xyz".into(),
            available: vec![],
            suggestion: None,
            location: SourceLocation::new("test.ts", 3, Some(10)),
        };
        let result = FixSuggestionServiceImpl
            .suggest_fix(&failure, &ctx)
            .await
            .unwrap();
        assert!(result.is_none());
    }

    // -----------------------------------------------------------------------
    // WrongArgCount Suggestions
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_wrong_arg_count_missing_args() {
        let failure = TemplateFailure::WrongArgCount {
            function: "calculate".into(),
            expected: 3,
            actual: 1,
            location: SourceLocation::new("test.ts", 5, None),
        };
        let result = FixSuggestionServiceImpl
            .suggest_fix(&failure, &SourceContext::empty())
            .await
            .unwrap();
        assert!(result.is_some());
        let sug = result.unwrap();
        assert!(sug.contains("missing 2 arguments"));
    }

    #[tokio::test]
    async fn test_wrong_arg_count_extra_args() {
        let failure = TemplateFailure::WrongArgCount {
            function: "add".into(),
            expected: 2,
            actual: 4,
            location: SourceLocation::new("test.ts", 5, None),
        };
        let result = FixSuggestionServiceImpl
            .suggest_fix(&failure, &SourceContext::empty())
            .await
            .unwrap();
        assert!(result.is_some());
        let sug = result.unwrap();
        assert!(sug.contains("Remove 2 extra arguments"));
    }

    #[tokio::test]
    async fn test_wrong_arg_count_single_extra() {
        let failure = TemplateFailure::WrongArgCount {
            function: "add".into(),
            expected: 2,
            actual: 3,
            location: SourceLocation::new("test.ts", 5, None),
        };
        let result = FixSuggestionServiceImpl
            .suggest_fix(&failure, &SourceContext::empty())
            .await
            .unwrap();
        let sug = result.unwrap();
        assert!(sug.contains("1 extra argument")); // singular
    }

    // -----------------------------------------------------------------------
    // TypeMismatch Suggestions
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_type_mismatch_with_source_context() {
        let ctx = ctx_with_symbols("test.ts", &["string", "number", "Task"]);
        let failure = TemplateFailure::TypeMismatch {
            expected: "Task".into(),
            actual: "string".into(),
            location: SourceLocation::new("test.ts", 10, Some(5)),
        };
        let result = FixSuggestionServiceImpl
            .suggest_fix(&failure, &ctx)
            .await
            .unwrap();
        assert!(result.is_some());
        let sug = result.unwrap();
        assert!(sug.contains("Task"));
        assert!(sug.contains("string"));
    }

    #[tokio::test]
    async fn test_type_mismatch_no_context() {
        let failure = TemplateFailure::TypeMismatch {
            expected: "User".into(),
            actual: "string".into(),
            location: SourceLocation::new("test.ts", 10, Some(5)),
        };
        let result = FixSuggestionServiceImpl
            .suggest_fix(&failure, &SourceContext::empty())
            .await
            .unwrap();
        assert!(result.is_some());
        let sug = result.unwrap();
        assert!(sug.contains("User"));
        assert!(sug.contains("string"));
    }

    // -----------------------------------------------------------------------
    // CompileError Suggestions
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_compile_error_ts1005() {
        let failure = TemplateFailure::CompileError {
            code: "TS1005".into(),
            message: "';' expected.".into(),
            location: SourceLocation::new("test.ts", 1, None),
        };
        let result = FixSuggestionServiceImpl
            .suggest_fix(&failure, &SourceContext::empty())
            .await
            .unwrap();
        assert!(result.is_some());
        assert!(result.unwrap().contains("semicolon"));
    }

    #[tokio::test]
    async fn test_compile_error_unknown_code() {
        let failure = TemplateFailure::CompileError {
            code: "TS9999".into(),
            message: "Unknown error".into(),
            location: SourceLocation::new("test.ts", 1, None),
        };
        let result = FixSuggestionServiceImpl
            .suggest_fix(&failure, &SourceContext::empty())
            .await
            .unwrap();
        assert!(result.is_some());
    }

    // -----------------------------------------------------------------------
    // AssertionFailure Suggestions
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_assertion_failure_short_values() {
        let failure = TemplateFailure::AssertionFailure {
            test_name: "should add".into(),
            expected: "5".into(),
            received: "3".into(),
            location: SourceLocation::new("test.ts", 20, Some(1)),
        };
        let result = FixSuggestionServiceImpl
            .suggest_fix(&failure, &SourceContext::empty())
            .await
            .unwrap();
        assert!(result.is_some());
        let sug = result.unwrap();
        assert!(sug.contains("should add"));
        assert!(sug.contains("expected '5' but received '3'"));
    }

    #[tokio::test]
    async fn test_assertion_failure_long_values() {
        let long_val = "a".repeat(100);
        let failure = TemplateFailure::AssertionFailure {
            test_name: "test".into(),
            expected: long_val.clone(),
            received: "b".repeat(100),
            location: SourceLocation::new("test.ts", 20, Some(1)),
        };
        let result = FixSuggestionServiceImpl
            .suggest_fix(&failure, &SourceContext::empty())
            .await
            .unwrap();
        assert!(result.is_some());
        let sug = result.unwrap();
        // Long values shouldn't be printed inline
        assert!(!sug.contains(&long_val));
        assert!(sug.contains("expected value does not match"));
    }

    // -----------------------------------------------------------------------
    // TestFailure Suggestions
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_test_failure_with_message() {
        let failure = TemplateFailure::TestFailure {
            test_name: "should complete".into(),
            message: "TimeoutError: test exceeded 5000ms".into(),
            location: None,
        };
        let result = FixSuggestionServiceImpl
            .suggest_fix(&failure, &SourceContext::empty())
            .await
            .unwrap();
        assert!(result.is_some());
        let sug = result.unwrap();
        assert!(sug.contains("should complete"));
        assert!(sug.contains("TimeoutError"));
    }

    // -----------------------------------------------------------------------
    // Batch Suggestion Tests
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_batch_suggestions() {
        let ctx = ctx_with_symbols("test.ts", &["add", "remove"]);
        let failures = vec![
            TemplateFailure::MissingSymbol {
                symbol: "addTask".into(),
                available: vec!["add".into()],
                suggestion: None,
                location: SourceLocation::new("test.ts", 3, Some(10)),
            },
            TemplateFailure::CompileError {
                code: "TS1005".into(),
                message: "';' expected.".into(),
                location: SourceLocation::new("test.ts", 5, None),
            },
        ];

        let results = FixSuggestionServiceImpl
            .suggest_fixes_batch(&failures, &ctx)
            .await
            .unwrap();

        assert_eq!(results.len(), 2);
        assert!(results[0].1.is_some()); // MissingSymbol has suggestion
        assert!(results[1].1.is_some()); // CompileError has suggestion
    }

    #[tokio::test]
    async fn test_batch_suggestions_empty_input() {
        let results = FixSuggestionServiceImpl
            .suggest_fixes_batch(&[], &SourceContext::empty())
            .await
            .unwrap();

        assert!(results.is_empty());
    }

    // -----------------------------------------------------------------------
    // Find Best Match Tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_find_best_match_exact_substring() {
        let candidates = vec!["add".to_string(), "remove".to_string()];
        let result = FixSuggestionServiceImpl::find_best_match_in_list("addTask", &candidates);
        assert_eq!(result, Some("add"));
    }

    #[test]
    fn test_find_best_match_case_insensitive() {
        let candidates = vec!["Add".to_string(), "Remove".to_string()];
        let result = FixSuggestionServiceImpl::find_best_match_in_list("addTask", &candidates);
        assert_eq!(result, Some("Add"));
    }

    #[test]
    fn test_find_best_match_first_letter() {
        let candidates = vec!["abc".to_string(), "xyz".to_string()];
        let result = FixSuggestionServiceImpl::find_best_match_in_list("add", &candidates);
        assert_eq!(result, Some("abc"));
    }

    #[test]
    fn test_find_best_match_shared_prefix() {
        let candidates = vec!["addItem".to_string(), "remove".to_string()];
        let result = FixSuggestionServiceImpl::find_best_match_in_list("addTask", &candidates);
        assert_eq!(result, Some("addItem"));
    }

    #[test]
    fn test_find_best_match_empty_list() {
        let candidates: Vec<String> = vec![];
        let result =
            FixSuggestionServiceImpl::find_best_match_in_list("anything", &candidates);
        assert!(result.is_none());
    }

    #[test]
    fn test_find_best_match_no_match() {
        let candidates = vec!["abc".to_string(), "def".to_string()];
        let result =
            FixSuggestionServiceImpl::find_best_match_in_list("xyz", &candidates);
        // "xyz" and "abc" share 'x'≠'a' first letter diff, no substring match
        // But they share zero leading chars
        assert_eq!(result, None);
    }
}
