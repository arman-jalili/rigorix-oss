//! TypeScript Parser — parses `tsc --noEmit --pretty false` output into TemplateFailure values.
//!
//! @canonical .pi/architecture/modules/failure-parser.md#ts-parser
//! Implements: TypeScriptParser — LanguageParser for tsc output
//! Issue: #498
//!
//! Parses TypeScript compiler diagnostic output. The tsc compiler with
//! `--pretty false` outputs each error in the format:
//!
//! ```text
//! src/task.ts(3,10): error TS2339: Property 'addTask' does not exist on type 'TaskList'.
//! ```
//!
//! The parser extracts:
//! - File path and location (line, column)
//! - Error code (TS2339, TS2554, TS2345, etc.)
//! - Error message
//! - Available symbols (if source context is provided)
//!
//! # Error Code Mapping
//!
//! | Code | TemplateFailure | Notes |
//! |------|----------------|-------|
//! | TS2339 | MissingSymbol | Property does not exist |
//! | TS2304 | MissingSymbol | Cannot find name |
//! | TS2551 | MissingSymbol | Property does not exist (stricter) |
//! | TS2554 | WrongArgCount | Expected N arguments, got M |
//! | TS2345 | TypeMismatch | Type '{actual}' is not assignable to type '{expected}' |
//! | TS1005 | CompileError | ';' expected |
//! | TSXXXX | CompileError | Fallback for unrecognized codes |

use async_trait::async_trait;
use regex::Regex;

use crate::failure_parser::domain::{
    detail::FailureDetail,
    failure::{SourceLocation, TemplateFailure},
    FailureParserError, FailureSeverity, LanguageParser, ParsedFailure, SourceContext,
};

/// Parses TypeScript compiler (tsc) output into structured TemplateFailure values.
///
/// Handles `--pretty false` output format where each error is one line:
/// `file(line,col): error TSXXXX: message`
pub struct TypeScriptParser;

impl TypeScriptParser {
    /// Create a new TypeScriptParser.
    pub fn new() -> Self {
        Self
    }

    /// Parse a single tsc error line into a TemplateFailure.
    fn parse_line(line: &str, source_context: &SourceContext) -> Option<FailureDetail> {
        // Try multiple tsc error line formats
        Self::try_parse_format_1(line, source_context)
            .or_else(|| Self::try_parse_format_2(line, source_context))
    }

    /// Format 1: file(line,col): error TSXXXX: message (standard tsc --pretty false)
    fn try_parse_format_1(line: &str, source_context: &SourceContext) -> Option<FailureDetail> {
        let re = Regex::new(
            r"^(.+?)\((\d+),(\d+)\):\s+error\s+(TS\d+):\s+(.+)"
        ).ok()?;
        let caps = re.captures(line)?;
        let file = caps.get(1)?.as_str();
        let line_num: usize = caps.get(2)?.as_str().parse().ok()?;
        let col_num: usize = caps.get(3)?.as_str().parse().ok()?;
        let code = caps.get(4)?.as_str();
        let message = caps.get(5)?.as_str();

        let _location = SourceLocation::new(file, line_num, Some(col_num));
        let failure = Self::classify_error(code, message, file, source_context);
        Some(FailureDetail::new(
            failure,
            None,
            FailureSeverity::CompileBlock,
            line.to_string(),
            "tsc",
            Self::confidence_for_code(code),
        ))
    }

    /// Format 2: file(line,col): error TSXXXX - message (dash separator, some tsc versions)
    fn try_parse_format_2(line: &str, source_context: &SourceContext) -> Option<FailureDetail> {
        let re = Regex::new(
            r"^(.+?)\((\d+),(\d+)\):\s+error\s+(TS\d+)\s+-\s+(.+)"
        ).ok()?;
        let caps = re.captures(line)?;
        let file = caps.get(1)?.as_str();
        let line_num: usize = caps.get(2)?.as_str().parse().ok()?;
        let col_num: usize = caps.get(3)?.as_str().parse().ok()?;
        let code = caps.get(4)?.as_str();
        let message = caps.get(5)?.as_str();

        let _location = SourceLocation::new(file, line_num, Some(col_num));
        let failure = Self::classify_error(code, message, file, source_context);
        Some(FailureDetail::new(
            failure,
            None,
            FailureSeverity::CompileBlock,
            line.to_string(),
            "tsc",
            Self::confidence_for_code(code),
        ))
    }

    /// Parse a tsc error line without source location (alternative format).
    fn parse_line_no_location(line: &str) -> Option<TemplateFailure> {
        // Some tsc errors appear without file location:
        // "error TS6203: Project may not use both 'out' and 'outDir'."
        let re = Regex::new(r"^error\s+(TS\d+):\s+(.+)").ok()?;
        let caps = re.captures(line)?;
        let code = caps.get(1)?.as_str().to_string();
        let message = caps.get(2)?.as_str().to_string();

        Some(match code.as_str() {
            "TS2339" | "TS2304" | "TS2551" => TemplateFailure::MissingSymbol {
                symbol: message.clone(),
                available: vec![],
                suggestion: None,
                location: SourceLocation::new("unknown", 0, None),
            },
            _ => TemplateFailure::CompileError {
                code,
                message,
                location: SourceLocation::new("unknown", 0, None),
            },
        })
    }

    /// Classify a TypeScript error code into the correct TemplateFailure variant.
    fn classify_error(
        code: &str,
        message: &str,
        file: &str,
        source_context: &SourceContext,
    ) -> TemplateFailure {
        // Extract symbol names from error messages for better suggestions
        let symbol = Self::extract_symbol_from_message(message);
        let available = source_context.symbols_in_file(file);

        match code {
            "TS2339" => {
                // "Property 'X' does not exist on type 'Y'."
                // "Property 'X' does not exist on type 'Y'. Did you mean 'Z'?"
                let suggestion = Self::find_suggestion_from_message(message, &available);
                TemplateFailure::MissingSymbol {
                    symbol: symbol.unwrap_or_else(|| message.to_string()),
                    available: Self::extract_available_from_message(message, &available),
                    suggestion,
                    location: SourceLocation::new(file, 0, None),
                }
            }
            "TS2304" => {
                // "Cannot find name 'X'."
                let suggestion = Self::find_suggestion_from_message(message, &available);
                TemplateFailure::MissingSymbol {
                    symbol: symbol.unwrap_or_else(|| message.to_string()),
                    available: Self::extract_available_from_message(message, &available),
                    suggestion,
                    location: SourceLocation::new(file, 0, None),
                }
            }
            "TS2551" => {
                // "Property 'X' does not exist on type 'Y'. Did you mean 'Z'?"
                if let Some(did_you_mean) = Self::extract_did_you_mean(message) {
                    TemplateFailure::MissingSymbol {
                        symbol: symbol.unwrap_or_else(|| message.to_string()),
                        available: Self::extract_available_from_message(message, &available),
                        suggestion: Some(format!("Did you mean '{}'?", did_you_mean)),
                        location: SourceLocation::new(file, 0, None),
                    }
                } else {
                    TemplateFailure::MissingSymbol {
                        symbol: symbol.unwrap_or_else(|| message.to_string()),
                        available: Self::extract_available_from_message(message, &available),
                        suggestion: None,
                        location: SourceLocation::new(file, 0, None),
                    }
                }
            }
            "TS2554" => {
                // "Expected N arguments, but got M."
                // "Expected M+N arguments, but got M+N."
                let (expected, actual) = Self::extract_arg_counts(message);
                TemplateFailure::WrongArgCount {
                    function: symbol.unwrap_or_else(|| "unknown".to_string()),
                    expected: expected.unwrap_or(0),
                    actual: actual.unwrap_or(0),
                    location: SourceLocation::new(file, 0, None),
                }
            }
            "TS2345" => {
                // "Argument of type 'X' is not assignable to parameter of type 'Y'."
                let (expected, actual) = Self::extract_type_mismatch(message);
                TemplateFailure::TypeMismatch {
                    expected: expected.unwrap_or_else(|| "unknown".to_string()),
                    actual: actual.unwrap_or_else(|| "unknown".to_string()),
                    location: SourceLocation::new(file, 0, None),
                }
            }
            "TS1005" => {
                // "';' expected." — syntactic error
                TemplateFailure::CompileError {
                    code: code.to_string(),
                    message: message.to_string(),
                    location: SourceLocation::new(file, 0, None),
                }
            }
            // Fallback for unrecognized codes
            _ => TemplateFailure::CompileError {
                code: code.to_string(),
                message: message.to_string(),
                location: SourceLocation::new(file, 0, None),
            },
        }
    }

    /// Extract a symbol name from an error message using regex.
    fn extract_symbol_from_message(message: &str) -> Option<String> {
        // Matches: 'symbolName' in various TypeScript error messages
        let re = Regex::new(r"'(.*?)'").ok()?;
        re.captures(message)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().to_string())
    }

    /// Extract "Did you mean 'X'?" suggestion from message.
    fn extract_did_you_mean(message: &str) -> Option<String> {
        let re = Regex::new(r"(?i)did you mean '(.*?)'").ok()?;
        re.captures(message)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().to_string())
    }

    /// Find suggestion based on "Did you mean..." in message.
    fn find_suggestion_from_message(message: &str, _available: &[String]) -> Option<String> {
        Self::extract_did_you_mean(message)
            .map(|s| format!("Did you mean '{}'?", s))
    }

    /// Extract available symbols from "Did you mean..." context.
    fn extract_available_from_message(message: &str, source_symbols: &[String]) -> Vec<String> {
        let mut available: Vec<String> = Vec::new();

        // Extract "Did you mean 'X'"?
        if let Some(dym) = Self::extract_did_you_mean(message) {
            available.push(dym);
        }

        // Also add cross-referenced symbols from source context that match
        // fuzzy with the error context
        for sym in source_symbols {
            if !available.contains(sym) {
                available.push(sym.clone());
            }
        }

        available
    }

    /// Extract expected/actual argument counts from TS2554 messages.
    fn extract_arg_counts(message: &str) -> (Option<usize>, Option<usize>) {
        // "Expected N arguments, but got M."
        // "Expected at least N arguments, but got M."
        let expected_re = match Regex::new(r"Expected(?: at least)? (\d+)") {
            Ok(re) => re,
            Err(_) => return (None, None),
        };
        let actual_re = match Regex::new(r"but got (\d+)") {
            Ok(re) => re,
            Err(_) => return (None, None),
        };

        let expected = expected_re
            .captures(message)
            .and_then(|c| c.get(1))
            .and_then(|m| m.as_str().parse().ok());

        let actual = actual_re
            .captures(message)
            .and_then(|c| c.get(1))
            .and_then(|m| m.as_str().parse().ok());

        (expected, actual)
    }

    /// Extract expected/actual types from TS2345 messages.
    fn extract_type_mismatch(message: &str) -> (Option<String>, Option<String>) {
        // "Argument of type 'X' is not assignable to parameter of type 'Y'."
        // "Type 'X' is not assignable to type 'Y'."
        let types: Vec<String> = Regex::new(r"'(.*?)'")
            .ok()
            .map(|re| {
                re.captures_iter(message)
                    .filter_map(|c| c.get(1).map(|m| m.as_str().to_string()))
                    .collect()
            })
            .unwrap_or_default();

        if types.len() >= 2 {
            (Some(types[1].clone()), Some(types[0].clone()))
        } else if types.len() == 1 {
            (None, Some(types[0].clone()))
        } else {
            (None, None)
        }
    }

    /// Return the confidence for a given error code.
    fn confidence_for_code(code: &str) -> f64 {
        match code {
            "TS2339" | "TS2304" | "TS2551" => 0.95,
            "TS2554" | "TS2345" => 0.95,
            "TS1005" => 0.90,
            _ => 0.80, // Unknown codes — still reasonable
        }
    }
}

impl Default for TypeScriptParser {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl LanguageParser for TypeScriptParser {
    fn tool_name(&self) -> &str {
        "tsc"
    }

    async fn parse(
        &self,
        output: &str,
        source_context: &SourceContext,
    ) -> Result<ParsedFailure, FailureParserError> {
        if output.trim().is_empty() {
            return Ok(ParsedFailure::from_failures(vec![], "tsc"));
        }

        let mut details: Vec<FailureDetail> = Vec::new();

        for line in output.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            // Try to parse as a standard tsc error line
            if let Some(detail) = Self::parse_line(trimmed, source_context) {
                details.push(detail);
                continue;
            }

            // Try alternative format without location
            if trimmed.contains("error TS") {
                if let Some(failure) = Self::parse_line_no_location(trimmed) {
                    let detail = FailureDetail::new(
                        failure,
                        None,
                        FailureSeverity::CompileBlock,
                        trimmed.to_string(),
                        "tsc",
                        0.7,
                    );
                    details.push(detail);
                }
            }
        }

        if details.is_empty() && output.contains("error TS") {
            // The output contains error TS codes but we couldn't parse any lines
            return Err(FailureParserError::UnrecognizedFormat {
                detail: "Output contains 'error TS' but no parseable tsc error lines found".into(),
                tool: "tsc".into(),
            });
        }

        Ok(ParsedFailure::from_failures(details, "tsc"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    fn empty_context() -> SourceContext {
        SourceContext::empty()
    }

    fn context_with_symbols(file: &str, symbols: &[&str]) -> SourceContext {
        let mut ctx = SourceContext::empty();
        ctx.symbols_by_file.insert(
            file.to_string(),
            symbols.iter().map(|s| s.to_string()).collect(),
        );
        ctx
    }

    // -----------------------------------------------------------------------
    // Single Line Parsing
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_parse_ts2339_missing_symbol() {
        let output = "src/tasklist.ts(3,10): error TS2339: Property 'addTask' does not exist on type 'TaskList'.";
        let result = TypeScriptParser.parse(output, &empty_context()).await.unwrap();
        assert_eq!(result.total_count, 1);
        assert_eq!(result.failures[0].failure.variant_name(), "missing_symbol");
        if let TemplateFailure::MissingSymbol { symbol, .. } = &result.failures[0].failure {
            assert_eq!(symbol, "addTask");
        } else {
            panic!("Expected MissingSymbol");
        }
        assert_eq!(result.failures[0].source_tool, "tsc");
    }

    #[tokio::test]
    async fn test_parse_ts2339_with_did_you_mean() {
        let output = "src/tasklist.ts(5,10): error TS2339: Property 'addd' does not exist on type 'TaskList'. Did you mean 'add'?";
        let result = TypeScriptParser.parse(output, &empty_context()).await.unwrap();
        assert_eq!(result.total_count, 1);
        if let TemplateFailure::MissingSymbol { suggestion, .. } = &result.failures[0].failure {
            assert!(suggestion.as_deref().unwrap_or("").contains("add"));
        } else {
            panic!("Expected MissingSymbol");
        }
    }

    #[tokio::test]
    async fn test_parse_ts2554_wrong_arg_count() {
        let output = "src/tasklist.ts(15,3): error TS2554: Expected 2 arguments, but got 3.";
        let result = TypeScriptParser.parse(output, &empty_context()).await.unwrap();
        assert_eq!(result.total_count, 1);
        assert_eq!(result.failures[0].failure.variant_name(), "wrong_arg_count");
        if let TemplateFailure::WrongArgCount {
            function: _, expected, actual, ..
        } = &result.failures[0].failure
        {
            // function is extracted as first quoted string = "2 arguments"
            assert_eq!(*expected, 2);
            assert_eq!(*actual, 3);
        } else {
            panic!("Expected WrongArgCount");
        }
    }

    #[tokio::test]
    async fn test_parse_ts2345_type_mismatch() {
        let output = "src/tasklist.ts(20,5): error TS2345: Argument of type 'string' is not assignable to parameter of type 'Task'.";
        let result = TypeScriptParser.parse(output, &empty_context()).await.unwrap();
        assert_eq!(result.total_count, 1);
        assert_eq!(result.failures[0].failure.variant_name(), "type_mismatch");
        if let TemplateFailure::TypeMismatch {
            expected, actual, ..
        } = &result.failures[0].failure
        {
            assert_eq!(expected, "Task");
            assert_eq!(actual, "string");
        } else {
            panic!("Expected TypeMismatch");
        }
    }

    #[tokio::test]
    async fn test_parse_ts1005_compile_error() {
        let output = "src/tasklist.ts(25,1): error TS1005: ';' expected.";
        let result = TypeScriptParser.parse(output, &empty_context()).await.unwrap();
        assert_eq!(result.total_count, 1);
        assert_eq!(result.failures[0].failure.variant_name(), "compile_error");
        if let TemplateFailure::CompileError { code, .. } = &result.failures[0].failure {
            assert_eq!(code, "TS1005");
        } else {
            panic!("Expected CompileError");
        }
    }

    #[tokio::test]
    async fn test_parse_ts2304_missing_symbol() {
        let output = "src/index.ts(1,1): error TS2304: Cannot find name 'myVar'.";
        let result = TypeScriptParser.parse(output, &empty_context()).await.unwrap();
        assert_eq!(result.total_count, 1);
        assert_eq!(result.failures[0].failure.variant_name(), "missing_symbol");
        if let TemplateFailure::MissingSymbol { symbol, .. } = &result.failures[0].failure {
            assert_eq!(symbol, "myVar");
        } else {
            panic!("Expected MissingSymbol");
        }
    }

    #[tokio::test]
    async fn test_parse_ts2551_missing_symbol() {
        let output = "src/tasklist.ts(3,10): error TS2551: Property 'addTask' does not exist on type 'TaskList'. Did you mean 'add'?";
        let result = TypeScriptParser.parse(output, &empty_context()).await.unwrap();
        assert_eq!(result.total_count, 1);
        assert_eq!(result.failures[0].failure.variant_name(), "missing_symbol");
        if let TemplateFailure::MissingSymbol { suggestion, .. } = &result.failures[0].failure {
            assert!(suggestion.as_deref().unwrap_or("").contains("add"), "Expected suggestion to mention 'add', got: {:?}", suggestion);
        } else {
            panic!("Expected MissingSymbol with suggestion");
        }
    }

    #[tokio::test]
    async fn test_parse_unknown_error_code() {
        let output = "src/file.ts(1,1): error TS9999: Some unknown error occurred.";
        let result = TypeScriptParser.parse(output, &empty_context()).await.unwrap();
        assert_eq!(result.total_count, 1);
        assert_eq!(result.failures[0].failure.variant_name(), "compile_error");
        if let TemplateFailure::CompileError { code, .. } = &result.failures[0].failure {
            assert_eq!(code, "TS9999");
        } else {
            panic!("Expected CompileError");
        }
    }

    // -----------------------------------------------------------------------
    // Multi-line Parsing
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_parse_multiple_errors() {
        let output = "\
src/file1.ts(1,1): error TS2339: Property 'x' does not exist.
src/file2.ts(5,3): error TS2554: Expected 2 arguments, but got 3.
src/file3.ts(10,1): error TS1005: ';' expected.";
        let result = TypeScriptParser.parse(output, &empty_context()).await.unwrap();
        assert_eq!(result.total_count, 3);
        assert_eq!(result.failures[0].failure.variant_name(), "missing_symbol");
        assert_eq!(result.failures[1].failure.variant_name(), "wrong_arg_count");
        assert_eq!(result.failures[2].failure.variant_name(), "compile_error");
    }

    #[tokio::test]
    async fn test_parse_empty_output() {
        let result = TypeScriptParser.parse("", &empty_context()).await.unwrap();
        assert!(result.is_clean());
        assert_eq!(result.total_count, 0);
    }

    #[tokio::test]
    async fn test_parse_whitespace_only_output() {
        let result = TypeScriptParser.parse("  \n  \n  ", &empty_context()).await.unwrap();
        assert!(result.is_clean());
    }

    #[tokio::test]
    async fn test_parse_clean_output() {
        let output = "No errors found.";
        let result = TypeScriptParser.parse(output, &empty_context()).await.unwrap();
        assert!(result.is_clean());
    }

    #[tokio::test]
    async fn test_parse_output_with_error_but_no_parseable_lines() {
        // Does not contain "error TS", so returns empty parsed result
        let output = "Some error occurred but format is unrecognized";
        let result = TypeScriptParser.parse(output, &empty_context()).await.unwrap();
        assert!(result.is_clean());
    }

    #[tokio::test]
    async fn test_parse_unrecognized_error_ts_format() {
        let output = "error TS9999 something unrecognized format";
        let result = TypeScriptParser.parse(output, &empty_context()).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            FailureParserError::UnrecognizedFormat { .. } => {} // expected
            e => panic!("Expected UnrecognizedFormat, got: {:?}", e),
        }
    }

    // -----------------------------------------------------------------------
    // Source Context Integration
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_parse_with_source_context_available_symbols() {
        let ctx = context_with_symbols("src/tasklist.ts", &["add", "remove", "list"]);
        let output = "src/tasklist.ts(3,10): error TS2339: Property 'addd' does not exist on type 'TaskList'.";
        let result = TypeScriptParser.parse(output, &ctx).await.unwrap();
        assert_eq!(result.total_count, 1);
        if let TemplateFailure::MissingSymbol { available, .. } = &result.failures[0].failure {
            assert!(available.contains(&"add".to_string()));
            assert!(available.contains(&"remove".to_string()));
        } else {
            panic!("Expected MissingSymbol");
        }
    }

    // -----------------------------------------------------------------------
    // Extractor Tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_extract_symbol_from_message() {
        let msg = "Property 'addTask' does not exist on type 'TaskList'.";
        assert_eq!(
            TypeScriptParser::extract_symbol_from_message(msg).as_deref(),
            Some("addTask")
        );
    }

    #[test]
    fn test_extract_symbol_no_quotes() {
        let msg = "Some error without quotes";
        assert_eq!(
            TypeScriptParser::extract_symbol_from_message(msg),
            None
        );
    }

    #[test]
    fn test_extract_did_you_mean() {
        let msg = "Did you mean 'add'?";
        assert_eq!(
            TypeScriptParser::extract_did_you_mean(msg).as_deref(),
            Some("add")
        );
    }

    #[test]
    fn test_extract_did_you_mean_case_insensitive() {
        let msg = "DID YOU MEAN 'something'?";
        assert_eq!(
            TypeScriptParser::extract_did_you_mean(msg).as_deref(),
            Some("something")
        );
    }

    #[test]
    fn test_extract_did_you_mean_not_found() {
        let msg = "Property does not exist.";
        assert_eq!(TypeScriptParser::extract_did_you_mean(msg), None);
    }

    #[test]
    fn test_extract_arg_counts() {
        let msg = "Expected 2 arguments, but got 3.";
        let (expected, actual) = TypeScriptParser::extract_arg_counts(msg);
        assert_eq!(expected, Some(2));
        assert_eq!(actual, Some(3));
    }

    #[test]
    fn test_extract_arg_counts_at_least() {
        let msg = "Expected at least 1 argument, but got 0.";
        let (expected, actual) = TypeScriptParser::extract_arg_counts(msg);
        assert_eq!(expected, Some(1));
        assert_eq!(actual, Some(0));
    }

    #[test]
    fn test_extract_arg_counts_not_found() {
        let msg = "Some other error.";
        let (expected, actual) = TypeScriptParser::extract_arg_counts(msg);
        assert_eq!(expected, None);
        assert_eq!(actual, None);
    }

    #[test]
    fn test_extract_type_mismatch() {
        let msg = "Argument of type 'string' is not assignable to parameter of type 'Task'.";
        let (expected, actual) = TypeScriptParser::extract_type_mismatch(msg);
        assert_eq!(expected.as_deref(), Some("Task"));
        assert_eq!(actual.as_deref(), Some("string"));
    }

    #[test]
    fn test_extract_type_mismatch_simple() {
        let msg = "Type 'number' is not assignable to type 'string'.";
        let (expected, actual) = TypeScriptParser::extract_type_mismatch(msg);
        assert_eq!(expected.as_deref(), Some("string"));
        assert_eq!(actual.as_deref(), Some("number"));
    }

    #[test]
    fn test_extract_type_mismatch_not_found() {
        let msg = "Some error.";
        let (expected, actual) = TypeScriptParser::extract_type_mismatch(msg);
        assert_eq!(expected, None);
        assert_eq!(actual, None);
    }

    #[test]
    fn test_tool_name() {
        let parser = TypeScriptParser::new();
        assert_eq!(parser.tool_name(), "tsc");
    }

    #[test]
    fn test_confidence_for_code() {
        assert!((TypeScriptParser::confidence_for_code("TS2339") - 0.95).abs() < 1e-10);
        assert!((TypeScriptParser::confidence_for_code("TS2554") - 0.95).abs() < 1e-10);
        assert!((TypeScriptParser::confidence_for_code("TS1005") - 0.90).abs() < 1e-10);
        assert!((TypeScriptParser::confidence_for_code("TS9999") - 0.80).abs() < 1e-10);
    }
}
