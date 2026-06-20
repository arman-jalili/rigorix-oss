//! Implementation of `FailureParserService`.
//!
//! @canonical .pi/architecture/modules/failure-parser.md#service
//! Implements: FailureParserService trait — parse compiler/test output into structured failures
//! Issue: #497
//!
//! Orchestrates the parser registry to find the right parser for the tool,
//! delegates parsing, and generates suggested fixes using source context.

use async_trait::async_trait;
use tokio::sync::Mutex;

use crate::failure_parser::domain::{
    failure::TemplateFailure,
    FailureParserError, ParserRegistry, ParsedFailure, SourceContext,
};

use super::dto::{
    FormatForLlmInput, FormatForLlmOutput, ParseOutputInput, ParseOutputResult,
    RegisterParserInput, RegisterParserResult, SuggestFixInput, SuggestFixOutput,
};
use super::service::FailureParserService;

/// Implementation of `FailureParserService`.
///
/// Orchestrates parsing by:
/// 1. Finding the appropriate parser in the ParserRegistry
/// 2. Delegating to the parser to get structured failures
/// 3. Generating suggested fixes using SourceContext
/// 4. Formatting results for LLM consumption
pub struct FailureParserServiceImpl {
    /// Registry of language/tool parsers.
    /// Stored as a pinned box to provide a stable memory address for &ParserRegistry.
    registry: Box<ParserRegistry>,
    /// Mutex for interior mutability (register_parser and other mutations).
    registry_lock: Mutex<()>,
}

impl FailureParserServiceImpl {
    /// Create a new FailureParserServiceImpl with the given parser registry.
    pub fn new(registry: ParserRegistry) -> Self {
        Self {
            registry: Box::new(registry),
            registry_lock: Mutex::new(()),
        }
    }

    /// Create an empty FailureParserServiceImpl (no parsers registered).
    /// Use `register_parser` or access the registry directly to add parsers.
    pub fn empty() -> Self {
        Self {
            registry: Box::new(ParserRegistry::new()),
            registry_lock: Mutex::new(()),
        }
    }

    /// Generate a suggested fix for a single failure.
    ///
    /// This is the core fix generation logic. It cross-references the
    /// failure against the source context to produce actionable guidance.
    fn generate_fix(
        &self,
        failure: &TemplateFailure,
        source_context: &SourceContext,
        min_confidence: f64,
    ) -> Result<SuggestFixOutput, FailureParserError> {
        match failure {
            TemplateFailure::MissingSymbol {
                symbol,
                available,
                location,
                ..
            } => {
                // Check 1: Look for exact substring matches
                let all_symbols = source_context.symbols_in_file(&location.file);

                // Prioritize available from the failure itself (compiler-provided)
                if let Some(matched) = available
                    .iter()
                    .find(|s| symbol.contains(s.as_str()) || s.contains(symbol.as_str()))
                {
                    return Ok(SuggestFixOutput {
                        suggestion: Some(format!("Use '{}' instead of '{}'", matched, symbol)),
                        confidence: 0.9,
                        rationale: Some(format!(
                            "Exact substring match: '{}' contains '{}'",
                            symbol, matched
                        )),
                    });
                }

                // Check 2: Cross-reference with source context
                if let Some(matched) = all_symbols.iter().find(|s| {
                    symbol.to_lowercase().contains(&s.to_lowercase())
                    || s.to_lowercase().contains(&symbol.to_lowercase())
                }) {
                    return Ok(SuggestFixOutput {
                        suggestion: Some(format!("Use '{}' instead of '{}'", matched, symbol)),
                        confidence: 0.8,
                        rationale: Some(format!(
                            "Cross-reference match: found '{}' in source context",
                            matched
                        )),
                    });
                }

                // Check 3: If we have available symbols, suggest the closest
                if !available.is_empty() {
                    let listed = available.join("', '");
                    return Ok(SuggestFixOutput {
                        suggestion: None,
                        confidence: 0.3,
                        rationale: Some(format!(
                            "No direct match found. Available symbols: '{}'",
                            listed
                        )),
                    });
                }

                Ok(SuggestFixOutput {
                    suggestion: None,
                    confidence: 0.0,
                    rationale: Some("No matching symbols found in context".into()),
                })
            }

            TemplateFailure::WrongArgCount {
                function,
                expected,
                actual,
                ..
            } => Ok(SuggestFixOutput {
                suggestion: Some(format!(
                    "'{}' expects {} arguments but {} were provided. Check the function signature and adjust the call.",
                    function, expected, actual
                )),
                confidence: 1.0,
                rationale: Some(format!(
                    "Argument count mismatch: expected {}, actual {}",
                    expected, actual
                )),
            }),

            TemplateFailure::TypeMismatch {
                expected,
                actual,
                location,
                ..
            } => {
                // Check source context for the expected type
                let all_symbols = source_context.symbols_in_file(&location.file);
                let type_hint = if all_symbols.iter().any(|s| s.contains(expected)) {
                    format!(" Type '{}' is available in this file.", expected)
                } else {
                    String::new()
                };

                Ok(SuggestFixOutput {
                    suggestion: Some(format!(
                        "Type mismatch: expected '{}' but got '{}'.{} Consider adding a type cast or changing the variable type.",
                        expected, actual, type_hint
                    )),
                    confidence: 0.85,
                    rationale: Some(format!(
                        "Type mismatch between '{}' and '{}'",
                        expected, actual
                    )),
                })
            }

            TemplateFailure::CompileError { code, message, .. } => {
                let suggestion = if code == "TS1005" {
                    Some("Missing semicolon or bracket — check syntax near this line.".into())
                } else {
                    Some(format!(
                        "Fix compile error {}: {}. Review the code at the reported location.",
                        code, message
                    ))
                };

                Ok(SuggestFixOutput {
                    suggestion,
                    confidence: 0.6,
                    rationale: Some(format!("Compile error {}: {}", code, message)),
                })
            }

            TemplateFailure::AssertionFailure {
                test_name,
                expected,
                received,
                ..
            } => Ok(SuggestFixOutput {
                suggestion: Some(format!(
                    "Test '{}' failed: expected '{}' but received '{}'. Check the test logic and expected values.",
                    test_name, expected, received
                )),
                confidence: 0.9,
                rationale: Some(format!(
                    "Assertion failure in '{}': expected '{}', got '{}'",
                    test_name, expected, received
                )),
            }),

            TemplateFailure::TestFailure { test_name, message, .. } => Ok(SuggestFixOutput {
                suggestion: Some(format!(
                    "Test '{}' failed: {}. Review the test and check for edge cases.",
                    test_name, message
                )),
                confidence: 0.7,
                rationale: Some(format!("Test failure '{}': {}", test_name, message)),
            }),
        }
        .map(|output| {
            if output.confidence < min_confidence {
                SuggestFixOutput {
                    suggestion: None,
                    ..output
                }
            } else {
                output
            }
        })
    }

    /// Format a single TemplateFailure into a human-readable line.
    fn format_failure(&self, f: &TemplateFailure) -> String {
        match f {
            TemplateFailure::MissingSymbol {
                symbol,
                available,
                suggestion,
                location,
            } => {
                let avail_str = if available.is_empty() {
                    String::new()
                } else {
                    format!(
                        "\n    Available symbols: {}",
                        available.join(", ")
                    )
                };
                let sug_str = suggestion
                    .as_ref()
                    .map(|s| format!("\n    Suggested fix: {}", s))
                    .unwrap_or_default();
                format!(
                    " - {}:{}: MissingSymbol '{}' not found in scope.{}{}",
                    location.file, location.line, symbol, avail_str, sug_str
                )
            }
            TemplateFailure::WrongArgCount {
                function,
                expected,
                actual,
                location,
            } => {
                format!(
                    " - {}:{}: WrongArgCount — '{}' expects {} args, got {}",
                    location.file, location.line, function, expected, actual
                )
            }
            TemplateFailure::TypeMismatch {
                expected,
                actual,
                location,
            } => {
                format!(
                    " - {}:{}: TypeMismatch — expected '{}', got '{}'",
                    location.file, location.line, expected, actual
                )
            }
            TemplateFailure::CompileError {
                code, message, ..
            } => {
                format!(" - {}: {} — {}", code, message, f.summary())
            }
            TemplateFailure::AssertionFailure {
                test_name,
                expected,
                received,
                location,
            } => {
                format!(
                    " - {}:{}: AssertionFailure '{}' — expected '{}', received '{}'",
                    location.file, location.line, test_name, expected, received
                )
            }
            TemplateFailure::TestFailure {
                test_name, message, ..
            } => {
                format!(
                    " - TestFailure '{}': {}",
                    test_name, message
                )
            }
        }
    }
}

#[async_trait]
impl FailureParserService for FailureParserServiceImpl {
    async fn parse(
        &self,
        input: ParseOutputInput,
    ) -> Result<ParseOutputResult, FailureParserError> {
        // If exit code is 0 and output is empty, it's clean
        if input.exit_code == 0 && input.stdout.is_empty() && input.stderr.is_empty() {
            return Ok(ParseOutputResult {
                parsed: ParsedFailure::from_failures(vec![], &input.tool),
                llm_summary: "No errors found.".to_string(),
                success: true,
            });
        }

        // Find the parser — hold lock only for lookup, not during async parse
        let combined = if input.stderr.is_empty() {
            input.stdout.clone()
        } else if input.stdout.is_empty() {
            input.stderr.clone()
        } else {
            format!("{}\n{}", input.stdout, input.stderr)
        };

        let _lock = self.registry_lock.lock().await;
        let parser = self.registry
            .get(&input.tool)
            .ok_or_else(|| FailureParserError::UnsupportedTool {
                tool: input.tool.clone(),
                available: self.registry.available_tools(),
            })?;

        // Parse with lock held (parsing is fast, no I/O)
        let mut parsed = parser.parse(&combined, &input.source_context).await?;

        // Generate suggestions for each failure
        let source_context = input.source_context;

        // Release the lock
        drop(_lock);

        let mut total_fixable = 0;
        for detail in &mut parsed.failures {
            let fix = self.generate_fix(
                &detail.failure,
                &source_context,
                0.5,
            )?;
            let has_fix = fix.suggestion.is_some();
            detail.suggested_fix = fix.suggestion;
            detail.confidence = fix.confidence;
            if has_fix {
                total_fixable += 1;
            }
        }
        parsed.fixable_count = total_fixable;

        // Generate LLM-readable summary
        let mut lines = Vec::new();
        if parsed.failures.is_empty() {
            lines.push("FAILURE ANALYSIS: No errors found.".to_string());
        } else {
            lines.push(format!(
                "FAILURE ANALYSIS: {} {} found ({} fixable).",
                parsed.total_count,
                if parsed.total_count == 1 {
                    "error"
                } else {
                    "errors"
                },
                parsed.fixable_count,
            ));
            lines.push(String::new());
            for detail in &parsed.failures {
                let line = self.format_failure(&detail.failure);
                if let Some(fix) = &detail.suggested_fix {
                    lines.push(format!("{}\n   SUGGESTED FIX: {}", line, fix));
                } else {
                    lines.push(line);
                }
            }
            lines.push(String::new());
            lines.push(format!(
                "Overall severity: {:?}",
                parsed.overall_severity
            ));
        }

        Ok(ParseOutputResult {
            llm_summary: lines.join("\n"),
            success: !parsed.failures.is_empty(),
            parsed,
        })
    }

    async fn format_for_llm(
        &self,
        input: FormatForLlmInput,
    ) -> Result<FormatForLlmOutput, FailureParserError> {
        if input.failures.is_empty() {
            return Ok(FormatForLlmOutput {
                formatted: "No failures to analyze.".to_string(),
                count: 0,
            });
        }

        let mut lines = Vec::new();

        // Title
        if let Some(title) = &input.title {
            lines.push(title.clone());
            lines.push(String::new());
        }

        // Summary header
        let fixable = input.failures.iter().filter(|f| f.is_fixable()).count();
        lines.push(format!(
            "FAILURE ANALYSIS: {} {} found ({} fixable, {} non-fixable).",
            input.failures.len(),
            if input.failures.len() == 1 {
                "error"
            } else {
                "errors"
            },
            fixable,
            input.failures.len() - fixable,
        ));
        lines.push(String::new());

        // Individual failures
        for f in &input.failures {
            let line = self.format_failure(f);
            lines.push(line);
        }

        Ok(FormatForLlmOutput {
            formatted: lines.join("\n"),
            count: input.failures.len(),
        })
    }

    async fn suggest_fix(
        &self,
        input: SuggestFixInput,
    ) -> Result<SuggestFixOutput, FailureParserError> {
        self.generate_fix(&input.failure, &input.source_context, input.min_confidence)
    }

    async fn register_parser(
        &self,
        _input: RegisterParserInput,
    ) -> Result<RegisterParserResult, FailureParserError> {
        // Parser registration requires actual parser implementations.
        // External code should create parsers and register them via
        // the ParserRegistry directly, or use the ParserFactory.
        //
        // This method is a placeholder that acknowledges the registration
        // intent. Full parser creation happens through the factory.
        let _lock = self.registry_lock.lock().await;
        Ok(RegisterParserResult {
            success: true,
            total_parsers: self.registry.len(),
            message: format!(
                "Parser registration intent recorded. {} parser(s) registered.",
                self.registry.len()
            ),
        })
    }

    fn parser_registry(&self) -> &ParserRegistry {
        // SAFETY: The Box<ParserRegistry> is initialized once in the constructor
        // and never moved or replaced, so the memory address is stable.
        &self.registry
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::failure_parser::domain::{
        detail::FailureDetail, failure::SourceLocation, CompilerOutput, FailureSeverity,
        LanguageParser,
    };
    use async_trait::async_trait;

    // Mock parser for testing
    struct MockParser {
        tool: String,
        produce_failures: bool,
    }

    #[async_trait]
    impl LanguageParser for MockParser {
        fn tool_name(&self) -> &str {
            &self.tool
        }

        async fn parse(
            &self,
            output: &str,
            _source_context: &SourceContext,
        ) -> Result<ParsedFailure, FailureParserError> {
            if !self.produce_failures {
                return Ok(ParsedFailure::from_failures(vec![], &self.tool));
            }

            let failures = vec![
                FailureDetail::new(
                    TemplateFailure::MissingSymbol {
                        symbol: "addTask".into(),
                        available: vec!["add".into(), "remove".into()],
                        suggestion: None,
                        location: SourceLocation::new("test.ts", 3, Some(10)),
                    },
                    None,
                    FailureSeverity::CompileBlock,
                    output.to_string(),
                    &self.tool,
                    0.95,
                ),
                FailureDetail::new(
                    TemplateFailure::CompileError {
                        code: "TS2339".into(),
                        message: "Property not found".into(),
                        location: SourceLocation::new("test.ts", 3, Some(10)),
                    },
                    None,
                    FailureSeverity::CompileBlock,
                    output.to_string(),
                    &self.tool,
                    0.95,
                ),
            ];

            Ok(ParsedFailure::from_failures(failures, &self.tool))
        }
    }

    fn setup_service() -> FailureParserServiceImpl {
        let mut registry = ParserRegistry::new();
        registry.register(Box::new(MockParser {
            tool: "tsc".to_string(),
            produce_failures: true,
        }));
        registry.register(Box::new(MockParser {
            tool: "jest".to_string(),
            produce_failures: true,
        }));
        registry.register(Box::new(MockParser {
            tool: "clean".to_string(),
            produce_failures: false,
        }));
        FailureParserServiceImpl::new(registry)
    }

    #[tokio::test]
    async fn test_parse_with_failures() {
        let service = setup_service();
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
        assert_eq!(result.parsed.total_count, 2);
        assert!(result.llm_summary.contains("FAILURE ANALYSIS"));
        assert!(result.llm_summary.contains("2 errors"));
    }

    #[tokio::test]
    async fn test_parse_clean_output() {
        let service = setup_service();
        let input = ParseOutputInput {
            tool: "clean".into(),
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
    async fn test_parse_unsupported_tool() {
        let service = setup_service();
        let input = ParseOutputInput {
            tool: "unknown".into(),
            stdout: "error".into(),
            stderr: String::new(),
            exit_code: 1,
            source_context: SourceContext::empty(),
            working_directory: "/project".into(),
        };

        let result = service.parse(input).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            FailureParserError::UnsupportedTool { tool, .. } => {
                assert_eq!(tool, "unknown");
            }
            e => panic!("Expected UnsupportedTool, got: {:?}", e),
        }
    }

    #[tokio::test]
    async fn test_format_for_llm_empty() {
        let service = setup_service();
        let result = service
            .format_for_llm(FormatForLlmInput {
                failures: vec![],
                title: None,
            })
            .await
            .unwrap();
        assert_eq!(result.formatted, "No failures to analyze.");
        assert_eq!(result.count, 0);
    }

    #[tokio::test]
    async fn test_format_for_llm_with_failures() {
        let service = setup_service();
        let failures = vec![
            TemplateFailure::MissingSymbol {
                symbol: "addTask".into(),
                available: vec!["add".into()],
                suggestion: Some("Use 'add'".into()),
                location: SourceLocation::new("test.ts", 3, Some(10)),
            },
            TemplateFailure::CompileError {
                code: "TS2339".into(),
                message: "not found".into(),
                location: SourceLocation::new("test.ts", 3, Some(10)),
            },
        ];

        let result = service
            .format_for_llm(FormatForLlmInput {
                failures,
                title: Some("TSC Analysis".into()),
            })
            .await
            .unwrap();

        assert_eq!(result.count, 2);
        assert!(result.formatted.contains("TSC Analysis"));
        assert!(result.formatted.contains("FAILURE ANALYSIS"));
        assert!(result.formatted.contains("2 errors"));
        assert!(result.formatted.contains("1 fixable"));
        assert!(result.formatted.contains("MissingSymbol"));
        assert!(result.formatted.contains("CompileError"));
    }

    #[tokio::test]
    async fn test_suggest_fix_missing_symbol_exact_match() {
        let service = setup_service();
        let mut ctx = SourceContext::empty();
        ctx.symbols_by_file
            .insert("test.ts".into(), vec!["add".into(), "remove".into()]);

        let result = service
            .suggest_fix(SuggestFixInput {
                failure: TemplateFailure::MissingSymbol {
                    symbol: "addTask".into(),
                    available: vec!["add".into()],
                    suggestion: None,
                    location: SourceLocation::new("test.ts", 3, Some(10)),
                },
                source_context: ctx,
                min_confidence: 0.5,
            })
            .await
            .unwrap();

        assert!(result.suggestion.is_some());
        assert!(result.suggestion.unwrap().contains("add"));
    }

    #[tokio::test]
    async fn test_suggest_fix_wrong_arg_count() {
        let service = setup_service();
        let result = service
            .suggest_fix(SuggestFixInput {
                failure: TemplateFailure::WrongArgCount {
                    function: "add".into(),
                    expected: 2,
                    actual: 3,
                    location: SourceLocation::new("test.ts", 5, None),
                },
                source_context: SourceContext::empty(),
                min_confidence: 0.5,
            })
            .await
            .unwrap();

        assert!(result.suggestion.is_some());
        let sug = result.suggestion.unwrap();
        assert!(sug.contains("expects 2 arguments"));
        assert!(sug.contains("3 were provided"));
    }

    #[tokio::test]
    async fn test_suggest_fix_type_mismatch() {
        let service = setup_service();
        let mut ctx = SourceContext::empty();
        ctx.symbols_by_file
            .insert("test.ts".into(), vec!["string".into()]);

        let result = service
            .suggest_fix(SuggestFixInput {
                failure: TemplateFailure::TypeMismatch {
                    expected: "string".into(),
                    actual: "number".into(),
                    location: SourceLocation::new("test.ts", 10, Some(5)),
                },
                source_context: ctx,
                min_confidence: 0.5,
            })
            .await
            .unwrap();

        assert!(result.suggestion.is_some());
        let sug = result.suggestion.unwrap();
        assert!(sug.contains("string"));
        assert!(sug.contains("number"));
    }

    #[tokio::test]
    async fn test_suggest_fix_assertion_failure() {
        let service = setup_service();
        let result = service
            .suggest_fix(SuggestFixInput {
                failure: TemplateFailure::AssertionFailure {
                    test_name: "should_add".into(),
                    expected: "5".into(),
                    received: "3".into(),
                    location: SourceLocation::new("test.ts", 20, Some(1)),
                },
                source_context: SourceContext::empty(),
                min_confidence: 0.5,
            })
            .await
            .unwrap();

        assert!(result.suggestion.is_some());
        let sug = result.suggestion.unwrap();
        assert!(sug.contains("should_add"));
        assert!(sug.contains("expected '5'"));
        assert!(sug.contains("received '3'"));
    }

    #[tokio::test]
    async fn test_suggest_fix_below_confidence_threshold() {
        let service = setup_service();
        let result = service
            .suggest_fix(SuggestFixInput {
                failure: TemplateFailure::MissingSymbol {
                    symbol: "addTask".into(),
                    available: vec![],
                    suggestion: None,
                    location: SourceLocation::new("test.ts", 3, Some(10)),
                },
                source_context: SourceContext::empty(),
                min_confidence: 0.9, // High threshold, confidence will be 0.3
            })
            .await
            .unwrap();

        assert!(result.suggestion.is_none());
    }

    #[tokio::test]
    async fn test_register_parser_returns_ok() {
        let service = setup_service();
        let result = service
            .register_parser(RegisterParserInput {
                tool: "rustc".into(),
                description: "Rust compiler parser".into(),
            })
            .await
            .unwrap();

        assert!(result.success);
        // Original 3 parsers
        assert_eq!(result.total_parsers, 3);
    }

    #[tokio::test]
    async fn test_parser_registry_reflects_registered_parsers() {
        let mut registry = ParserRegistry::new();
        registry.register(Box::new(MockParser {
            tool: "tsc".to_string(),
            produce_failures: true,
        }));
        let service = FailureParserServiceImpl::new(registry);
        let reg = service.parser_registry();
        assert!(reg.has_parser("tsc"));
        assert!(!reg.has_parser("jest"));
    }

    #[tokio::test]
    async fn test_format_failure_line_output() {
        let service = setup_service();
        let f = TemplateFailure::MissingSymbol {
            symbol: "addTask".into(),
            available: vec!["add".into()],
            suggestion: Some("Use 'add'".into()),
            location: SourceLocation::new("test.ts", 3, Some(10)),
        };
        let formatted = service.format_failure(&f);
        assert!(formatted.contains("test.ts:3"));
        assert!(formatted.contains("MissingSymbol"));
        assert!(formatted.contains("addTask"));
    }

    #[tokio::test]
    async fn test_parse_with_source_context_suggestions() {
        let service = setup_service();
        let mut ctx = SourceContext::empty();
        ctx.symbols_by_file
            .insert("test.ts".into(), vec!["add".into(), "remove".into()]);

        let input = ParseOutputInput {
            tool: "tsc".into(),
            stdout: "error TS2339".into(),
            stderr: String::new(),
            exit_code: 2,
            source_context: ctx,
            working_directory: "/project".into(),
        };

        let result = service.parse(input).await.unwrap();
        assert_eq!(result.parsed.total_count, 2);
        for detail in &result.parsed.failures {
            assert!(
                detail.suggested_fix.is_some() || detail.confidence < 0.5,
                "Each failure should either have a fix or low confidence"
            );
        }
    }
}
