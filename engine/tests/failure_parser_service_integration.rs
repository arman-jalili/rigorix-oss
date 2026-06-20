//! Integration tests for FailureParserService — parse raw output into structured failures.
//!
//! @canonical .pi/architecture/modules/failure-parser.md#service
//! Implements: FailureParserService integration tests
//! Issue: #497
//!
//! Tests the full lifecycle of the FailureParserService:
//! - Parsing raw compiler output with mock parsers
//! - Formatting failures for LLM consumption
//! - Suggested fix generation
//! - Parser registry interaction
//! - Empty/error input handling

use async_trait::async_trait;
use rigorix_engine::failure_parser::application::{
    FailureParserService, FailureParserServiceImpl, FormatForLlmInput, ParseOutputInput,
    RegisterParserInput, SuggestFixInput,
};
use rigorix_engine::failure_parser::domain::{
    FailureDetail, FailureParserError, FailureSeverity, LanguageParser, ParsedFailure,
    ParserRegistry, SourceContext, SourceLocation, TemplateFailure,
};

// ---------------------------------------------------------------------------
// Mock Parser — simulates TypeScript compiler output
// ---------------------------------------------------------------------------

struct MockTscParser;

#[async_trait]
impl LanguageParser for MockTscParser {
    fn tool_name(&self) -> &str {
        "tsc"
    }

    async fn parse(
        &self,
        output: &str,
        _source_context: &SourceContext,
    ) -> Result<ParsedFailure, FailureParserError> {
        if output.contains("TS2339") {
            Ok(ParsedFailure::from_failures(
                vec![FailureDetail::new(
                    TemplateFailure::MissingSymbol {
                        symbol: "addTask".into(),
                        available: vec!["add".into(), "list".into()],
                        suggestion: None,
                        location: SourceLocation::new("src/tasklist.ts", 10, Some(5)),
                    },
                    None,
                    FailureSeverity::CompileBlock,
                    "error TS2339: Property 'addTask' does not exist on type 'TaskList'".into(),
                    "tsc",
                    0.95,
                )],
                "tsc",
            ))
        } else if output.contains("TS2554") {
            Ok(ParsedFailure::from_failures(
                vec![FailureDetail::new(
                    TemplateFailure::WrongArgCount {
                        function: "add".into(),
                        expected: 2,
                        actual: 3,
                        location: SourceLocation::new("src/tasklist.ts", 15, Some(3)),
                    },
                    None,
                    FailureSeverity::CompileBlock,
                    "error TS2554: Expected 2 arguments, but got 3.".into(),
                    "tsc",
                    0.95,
                )],
                "tsc",
            ))
        } else if output.contains("TS2345") {
            Ok(ParsedFailure::from_failures(
                vec![FailureDetail::new(
                    TemplateFailure::TypeMismatch {
                        expected: "Task".into(),
                        actual: "string".into(),
                        location: SourceLocation::new("src/tasklist.ts", 20, Some(1)),
                    },
                    None,
                    FailureSeverity::CompileBlock,
                    "error TS2345: Argument of type 'string' is not assignable to parameter of type 'Task'".into(),
                    "tsc",
                    0.95,
                )],
                "tsc",
            ))
        } else if output.contains("TS1005") {
            Ok(ParsedFailure::from_failures(
                vec![FailureDetail::new(
                    TemplateFailure::CompileError {
                        code: "TS1005".into(),
                        message: "';' expected.".into(),
                        location: SourceLocation::new("src/tasklist.ts", 25, Some(1)),
                    },
                    None,
                    FailureSeverity::CompileBlock,
                    "error TS1005: ';' expected.".into(),
                    "tsc",
                    0.95,
                )],
                "tsc",
            ))
        } else if output.is_empty() {
            Ok(ParsedFailure::from_failures(vec![], "tsc"))
        } else {
            // Unrecognized format
            Err(FailureParserError::UnrecognizedFormat {
                detail: "Could not match any known pattern".into(),
                tool: "tsc".into(),
            })
        }
    }
}

struct MockJestParser;

#[async_trait]
impl LanguageParser for MockJestParser {
    fn tool_name(&self) -> &str {
        "jest"
    }

    async fn parse(
        &self,
        output: &str,
        _source_context: &SourceContext,
    ) -> Result<ParsedFailure, FailureParserError> {
        if output.contains("AssertionError") {
            Ok(ParsedFailure::from_failures(
                vec![FailureDetail::new(
                    TemplateFailure::AssertionFailure {
                        test_name: "should add a new task".into(),
                        expected: "5".into(),
                        received: "3".into(),
                        location: SourceLocation::new("tests/tasklist.test.ts", 42, Some(1)),
                    },
                    None,
                    FailureSeverity::TestBlock,
                    "AssertionError: expected 5, received 3".into(),
                    "jest",
                    0.95,
                )],
                "jest",
            ))
        } else if output.contains("Timeout") {
            Ok(ParsedFailure::from_failures(
                vec![FailureDetail::new(
                    TemplateFailure::TestFailure {
                        test_name: "should complete task".into(),
                        message: "TimeoutError: test exceeded 5000ms".into(),
                        location: None,
                    },
                    None,
                    FailureSeverity::TestBlock,
                    "TimeoutError: test exceeded 5000ms".into(),
                    "jest",
                    0.95,
                )],
                "jest",
            ))
        } else {
            Ok(ParsedFailure::from_failures(vec![], "jest"))
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn create_service() -> FailureParserServiceImpl {
    let mut registry = ParserRegistry::new();
    registry.register(Box::new(MockTscParser));
    registry.register(Box::new(MockJestParser));
    FailureParserServiceImpl::new(registry)
}

// ---------------------------------------------------------------------------
// Tests: Basic Parsing
// ---------------------------------------------------------------------------

#[tokio::test]
async fn integration_parse_tsc_missing_symbol() {
    let service = create_service();
    let input = ParseOutputInput {
        tool: "tsc".into(),
        stdout: "error TS2339: Property 'addTask' does not exist".into(),
        stderr: String::new(),
        exit_code: 2,
        source_context: SourceContext::empty(),
        working_directory: "/project".into(),
    };

    let result = service.parse(input).await.unwrap();
    assert!(result.success);
    assert_eq!(result.parsed.total_count, 1);
    assert_eq!(
        result.parsed.failures[0].failure.variant_name(),
        "missing_symbol"
    );
    assert_eq!(result.parsed.failures[0].source_tool, "tsc");
    assert!(result.llm_summary.contains("FAILURE ANALYSIS"));
}

#[tokio::test]
async fn integration_parse_tsc_wrong_arg_count() {
    let service = create_service();
    let input = ParseOutputInput {
        tool: "tsc".into(),
        stdout: "error TS2554: Expected 2 arguments, but got 3.".into(),
        stderr: String::new(),
        exit_code: 2,
        source_context: SourceContext::empty(),
        working_directory: "/project".into(),
    };

    let result = service.parse(input).await.unwrap();
    assert!(result.success);
    assert_eq!(
        result.parsed.failures[0].failure.variant_name(),
        "wrong_arg_count"
    );
}

#[tokio::test]
async fn integration_parse_tsc_type_mismatch() {
    let service = create_service();
    let input = ParseOutputInput {
        tool: "tsc".into(),
        stdout: "error TS2345: Argument of type 'string' is not assignable to parameter".into(),
        stderr: String::new(),
        exit_code: 2,
        source_context: SourceContext::empty(),
        working_directory: "/project".into(),
    };

    let result = service.parse(input).await.unwrap();
    assert!(result.success);
    assert_eq!(
        result.parsed.failures[0].failure.variant_name(),
        "type_mismatch"
    );
}

#[tokio::test]
async fn integration_parse_tsc_compile_error() {
    let service = create_service();
    let input = ParseOutputInput {
        tool: "tsc".into(),
        stdout: "error TS1005: ';' expected.".into(),
        stderr: String::new(),
        exit_code: 2,
        source_context: SourceContext::empty(),
        working_directory: "/project".into(),
    };

    let result = service.parse(input).await.unwrap();
    assert!(result.success);
    assert_eq!(
        result.parsed.failures[0].failure.variant_name(),
        "compile_error"
    );
    if let TemplateFailure::CompileError { code, .. } = &result.parsed.failures[0].failure {
        assert_eq!(code, "TS1005");
    } else {
        panic!("Expected CompileError variant");
    }
}

#[tokio::test]
async fn integration_parse_jest_assertion_failure() {
    let service = create_service();
    let input = ParseOutputInput {
        tool: "jest".into(),
        stdout: "AssertionError: expected 5, received 3".into(),
        stderr: String::new(),
        exit_code: 1,
        source_context: SourceContext::empty(),
        working_directory: "/project".into(),
    };

    let result = service.parse(input).await.unwrap();
    assert!(result.success);
    assert_eq!(
        result.parsed.failures[0].failure.variant_name(),
        "assertion_failure"
    );
    assert_eq!(result.parsed.failures[0].source_tool, "jest");
    assert_eq!(result.parsed.overall_severity, FailureSeverity::TestBlock);
}

#[tokio::test]
async fn integration_parse_jest_timeout() {
    let service = create_service();
    let input = ParseOutputInput {
        tool: "jest".into(),
        stdout: "TimeoutError: test exceeded 5000ms".into(),
        stderr: String::new(),
        exit_code: 1,
        source_context: SourceContext::empty(),
        working_directory: "/project".into(),
    };

    let result = service.parse(input).await.unwrap();
    assert!(result.success);
    assert_eq!(
        result.parsed.failures[0].failure.variant_name(),
        "test_failure"
    );
}

// ---------------------------------------------------------------------------
// Tests: Clean / Empty Output
// ---------------------------------------------------------------------------

#[tokio::test]
async fn integration_parse_clean_exit_code_0() {
    let service = create_service();
    let input = ParseOutputInput {
        tool: "tsc".into(),
        stdout: String::new(),
        stderr: String::new(),
        exit_code: 0,
        source_context: SourceContext::empty(),
        working_directory: "/project".into(),
    };

    let result = service.parse(input).await.unwrap();
    assert!(result.parsed.is_clean());
    assert_eq!(result.llm_summary, "No errors found.");
}

#[tokio::test]
async fn integration_parse_clean_with_output_exit_0() {
    let service = create_service();
    // tsc exits 0 when there are only warnings
    let input = ParseOutputInput {
        tool: "jest".into(),
        stdout: "PASS tests/ok.test.ts".into(),
        stderr: String::new(),
        exit_code: 0,
        source_context: SourceContext::empty(),
        working_directory: "/project".into(),
    };

    let result = service.parse(input).await.unwrap();
    // Jest parser doesn't match "PASS", so it returns empty
    assert!(result.parsed.is_clean());
}

// ---------------------------------------------------------------------------
// Tests: Error Handling
// ---------------------------------------------------------------------------

#[tokio::test]
async fn integration_parse_unsupported_tool() {
    let service = create_service();
    let input = ParseOutputInput {
        tool: "rustc".into(),
        stdout: "error[E0308]: type mismatch".into(),
        stderr: String::new(),
        exit_code: 1,
        source_context: SourceContext::empty(),
        working_directory: "/project".into(),
    };

    let result = service.parse(input).await;
    assert!(result.is_err());
    match result.unwrap_err() {
        FailureParserError::UnsupportedTool { tool, .. } => {
            assert_eq!(tool, "rustc");
        }
        e => panic!("Expected UnsupportedTool, got: {:?}", e),
    }
}

#[tokio::test]
async fn integration_parse_unrecognized_format() {
    let service = create_service();
    let input = ParseOutputInput {
        tool: "tsc".into(),
        stdout: "Random non-matching output".into(),
        stderr: String::new(),
        exit_code: 1,
        source_context: SourceContext::empty(),
        working_directory: "/project".into(),
    };

    let result = service.parse(input).await;
    assert!(result.is_err());
    match result.unwrap_err() {
        FailureParserError::UnrecognizedFormat { tool, .. } => {
            assert_eq!(tool, "tsc");
        }
        e => panic!("Expected UnrecognizedFormat, got: {:?}", e),
    }
}

// ---------------------------------------------------------------------------
// Tests: Format for LLM
// ---------------------------------------------------------------------------

#[tokio::test]
async fn integration_format_for_llm_multiple_failures() {
    let service = create_service();
    let input = FormatForLlmInput {
        failures: vec![
            TemplateFailure::MissingSymbol {
                symbol: "addTask".into(),
                available: vec!["add".into()],
                suggestion: Some("Use 'add'".into()),
                location: SourceLocation::new("test.ts", 3, Some(10)),
            },
            TemplateFailure::WrongArgCount {
                function: "add".into(),
                expected: 2,
                actual: 3,
                location: SourceLocation::new("test.ts", 15, Some(5)),
            },
        ],
        title: Some("TypeScript Compilation Analysis".into()),
    };

    let result = service.format_for_llm(input).await.unwrap();
    assert_eq!(result.count, 2);
    assert!(result.formatted.contains("TypeScript Compilation Analysis"));
    assert!(result.formatted.contains("FAILURE ANALYSIS"));
    assert!(result.formatted.contains("2 errors"));
    assert!(result.formatted.contains("MissingSymbol"));
    assert!(result.formatted.contains("WrongArgCount"));
}

// ---------------------------------------------------------------------------
// Tests: Suggest Fix with Source Context
// ---------------------------------------------------------------------------

#[tokio::test]
async fn integration_suggest_fix_with_source_context_symbols() {
    let service = create_service();
    let mut ctx = SourceContext::empty();
    ctx.symbols_by_file.insert(
        "src/tasklist.ts".into(),
        vec!["add".into(), "remove".into(), "list".into()],
    );

    let result = service
        .suggest_fix(SuggestFixInput {
            failure: TemplateFailure::MissingSymbol {
                symbol: "addItem".into(),
                available: vec!["add".into()],
                suggestion: None,
                location: SourceLocation::new("src/tasklist.ts", 10, Some(5)),
            },
            source_context: ctx,
            min_confidence: 0.5,
        })
        .await
        .unwrap();

    assert!(result.suggestion.is_some());
    let sug = result.suggestion.unwrap();
    assert!(sug.contains("add") || sug.contains("Use"));
}

// ---------------------------------------------------------------------------
// Tests: Parser Registry
// ---------------------------------------------------------------------------

#[tokio::test]
async fn integration_parser_registry_has_parsers() {
    let service = create_service();
    let registry = service.parser_registry();
    assert!(registry.has_parser("tsc"));
    assert!(registry.has_parser("jest"));
    assert!(!registry.has_parser("rustc"));
    assert_eq!(registry.len(), 2);
}

#[tokio::test]
async fn integration_register_parser_metadata() {
    let service = create_service();
    let result = service
        .register_parser(RegisterParserInput {
            tool: "rustc".into(),
            description: "Rust compiler parser".into(),
        })
        .await
        .unwrap();

    assert!(result.success);
    assert_eq!(result.total_parsers, 2);
}
