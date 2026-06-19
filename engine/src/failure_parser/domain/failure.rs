//! TemplateFailure — typed classification of template execution failures.
//!
//! @canonical .pi/architecture/modules/failure-parser.md#failure
//! Implements: Contract Freeze — TemplateFailure enum, SourceLocation
//! Issue: #495
//!
//! # Contract (Frozen)
//! - Six mutually exclusive failure categories
//! - Each variant carries structured context for LLM self-correction
//! - Implements `Clone`, `Debug`, `PartialEq`, `Eq` for testability
//! - Serialization support for eventing and API responses
//! - SourceLocation is the canonical location type used throughout

use serde::{Deserialize, Serialize};

/// Typed classification of why a template execution failed.
///
/// Each variant carries structured context suitable for feeding back
/// to the LLM for self-correction. Produced by `FailureParserService`
/// implementations (TypeScriptParser, JestParser, RustcParser, etc.).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TemplateFailure {
    /// A symbol (function, method, class, variable) was referenced but doesn't exist.
    MissingSymbol {
        /// The symbol that was referenced but not found.
        symbol: String,
        /// Available symbols in the same scope, if determinable.
        available: Vec<String>,
        /// Suggested replacement, if one closely matches.
        suggestion: Option<String>,
        /// Where the error occurred.
        location: SourceLocation,
    },

    /// Wrong number or type of arguments passed to a function/method.
    WrongArgCount {
        /// The function/method name.
        function: String,
        /// Expected number of arguments.
        expected: usize,
        /// Actual number of arguments provided.
        actual: usize,
        /// The call site.
        location: SourceLocation,
    },

    /// A type mismatch error.
    TypeMismatch {
        /// Expected type.
        expected: String,
        /// Actual type.
        actual: String,
        /// Where the error occurred.
        location: SourceLocation,
    },

    /// A compilation error that doesn't fit into more specific categories.
    CompileError {
        /// The compiler error code (e.g., TS2339, E0308).
        code: String,
        /// The error message.
        message: String,
        /// Where the error occurred.
        location: SourceLocation,
    },

    /// A test assertion failure.
    AssertionFailure {
        /// The test name.
        test_name: String,
        /// Expected value.
        expected: String,
        /// Received value.
        received: String,
        /// Where the error occurred.
        location: SourceLocation,
    },

    /// A generic test failure (test threw, timeout, etc.).
    TestFailure {
        /// The test name.
        test_name: String,
        /// The error message.
        message: String,
        /// Where the error occurred (may be None if location is unknown).
        location: Option<SourceLocation>,
    },
}

impl TemplateFailure {
    /// Returns the canonical snake_case type name for this failure variant.
    ///
    /// Used for serialization tags, logging, and event payloads.
    pub fn variant_name(&self) -> &'static str {
        match self {
            TemplateFailure::MissingSymbol { .. } => "missing_symbol",
            TemplateFailure::WrongArgCount { .. } => "wrong_arg_count",
            TemplateFailure::TypeMismatch { .. } => "type_mismatch",
            TemplateFailure::CompileError { .. } => "compile_error",
            TemplateFailure::AssertionFailure { .. } => "assertion_failure",
            TemplateFailure::TestFailure { .. } => "test_failure",
        }
    }

    /// Returns a human-readable one-line summary of this failure.
    ///
    /// Used for logging and LLM context summaries.
    pub fn summary(&self) -> String {
        match self {
            TemplateFailure::MissingSymbol {
                symbol, location, ..
            } => {
                format!(
                    "MissingSymbol '{}' at {}:{}",
                    symbol, location.file, location.line
                )
            }
            TemplateFailure::WrongArgCount {
                function,
                expected,
                actual,
                location,
            } => {
                format!(
                    "WrongArgCount '{}' at {}:{}: expected {} args, got {}",
                    function, location.file, location.line, expected, actual
                )
            }
            TemplateFailure::TypeMismatch {
                expected,
                actual,
                location,
            } => {
                format!(
                    "TypeMismatch at {}:{}: expected '{}', got '{}'",
                    location.file, location.line, expected, actual
                )
            }
            TemplateFailure::CompileError {
                code, message, ..
            } => {
                format!("CompileError {}: {}", code, message)
            }
            TemplateFailure::AssertionFailure {
                test_name,
                expected,
                received,
                ..
            } => {
                format!(
                    "AssertionFailure '{}': expected '{}', received '{}'",
                    test_name, expected, received
                )
            }
            TemplateFailure::TestFailure {
                test_name, message, ..
            } => {
                format!("TestFailure '{}': {}", test_name, message)
            }
        }
    }

    /// Returns `true` if this failure type is eligible for automatic retry
    /// after applying the suggested fix.
    ///
    /// MissingSymbol, WrongArgCount, and TypeMismatch are fixable.
    /// CompileError may be fixable depending on context.
    /// AssertionFailure and TestFailure typically require replanning.
    pub fn is_fixable(&self) -> bool {
        matches!(
            self,
            TemplateFailure::MissingSymbol { .. }
                | TemplateFailure::WrongArgCount { .. }
                | TemplateFailure::TypeMismatch { .. }
        )
    }
}

/// Location in source code where a failure occurred.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SourceLocation {
    /// File path relative to project root.
    pub file: String,
    /// Line number (1-indexed).
    pub line: usize,
    /// Column number (1-indexed, optional for non-column-specific errors).
    pub column: Option<usize>,
}

impl SourceLocation {
    /// Create a new SourceLocation.
    pub fn new(file: impl Into<String>, line: usize, column: Option<usize>) -> Self {
        Self {
            file: file.into(),
            line,
            column,
        }
    }

    /// Format as `file:line:column` (or `file:line` if column is None).
    pub fn to_compact(&self) -> String {
        match self.column {
            Some(col) => format!("{}:{}:{}", self.file, self.line, col),
            None => format!("{}:{}", self.file, self.line),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_missing_symbol_variant_name() {
        let f = TemplateFailure::MissingSymbol {
            symbol: "addTask".into(),
            available: vec!["add".into()],
            suggestion: Some("Use 'add' instead of 'addTask'".into()),
            location: SourceLocation::new("test.ts", 3, Some(10)),
        };
        assert_eq!(f.variant_name(), "missing_symbol");
    }

    #[test]
    fn test_wrong_arg_count_variant_name() {
        let f = TemplateFailure::WrongArgCount {
            function: "add".into(),
            expected: 2,
            actual: 3,
            location: SourceLocation::new("test.ts", 5, None),
        };
        assert_eq!(f.variant_name(), "wrong_arg_count");
    }

    #[test]
    fn test_type_mismatch_variant_name() {
        let f = TemplateFailure::TypeMismatch {
            expected: "string".into(),
            actual: "number".into(),
            location: SourceLocation::new("test.ts", 10, Some(5)),
        };
        assert_eq!(f.variant_name(), "type_mismatch");
    }

    #[test]
    fn test_compile_error_variant_name() {
        let f = TemplateFailure::CompileError {
            code: "TS2339".into(),
            message: "Property does not exist".into(),
            location: SourceLocation::new("test.ts", 3, Some(10)),
        };
        assert_eq!(f.variant_name(), "compile_error");
    }

    #[test]
    fn test_assertion_failure_variant_name() {
        let f = TemplateFailure::AssertionFailure {
            test_name: "should add task".into(),
            expected: "true".into(),
            received: "false".into(),
            location: SourceLocation::new("test.ts", 20, Some(1)),
        };
        assert_eq!(f.variant_name(), "assertion_failure");
    }

    #[test]
    fn test_test_failure_variant_name() {
        let f = TemplateFailure::TestFailure {
            test_name: "should compile".into(),
            message: "timeout".into(),
            location: None,
        };
        assert_eq!(f.variant_name(), "test_failure");
    }

    #[test]
    fn test_source_location_compact_with_column() {
        let loc = SourceLocation::new("src/lib.ts", 42, Some(7));
        assert_eq!(loc.to_compact(), "src/lib.ts:42:7");
    }

    #[test]
    fn test_source_location_compact_without_column() {
        let loc = SourceLocation::new("src/lib.ts", 42, None);
        assert_eq!(loc.to_compact(), "src/lib.ts:42");
    }

    #[test]
    fn test_is_fixable_missing_symbol() {
        let f = TemplateFailure::MissingSymbol {
            symbol: "x".into(),
            available: vec![],
            suggestion: None,
            location: SourceLocation::new("test.ts", 1, None),
        };
        assert!(f.is_fixable());
    }

    #[test]
    fn test_is_fixable_wrong_arg_count() {
        let f = TemplateFailure::WrongArgCount {
            function: "f".into(),
            expected: 1,
            actual: 2,
            location: SourceLocation::new("test.ts", 1, None),
        };
        assert!(f.is_fixable());
    }

    #[test]
    fn test_is_fixable_type_mismatch() {
        let f = TemplateFailure::TypeMismatch {
            expected: "string".into(),
            actual: "number".into(),
            location: SourceLocation::new("test.ts", 1, None),
        };
        assert!(f.is_fixable());
    }

    #[test]
    fn test_is_fixable_compile_error() {
        let f = TemplateFailure::CompileError {
            code: "TS1005".into(),
            message: "';' expected".into(),
            location: SourceLocation::new("test.ts", 1, None),
        };
        assert!(!f.is_fixable());
    }

    #[test]
    fn test_is_fixable_assertion_failure() {
        let f = TemplateFailure::AssertionFailure {
            test_name: "test".into(),
            expected: "true".into(),
            received: "false".into(),
            location: SourceLocation::new("test.ts", 1, None),
        };
        assert!(!f.is_fixable());
    }

    #[test]
    fn test_summary_missing_symbol() {
        let f = TemplateFailure::MissingSymbol {
            symbol: "addTask".into(),
            available: vec!["add".into()],
            suggestion: Some("Use 'add'".into()),
            location: SourceLocation::new("src/test.ts", 3, Some(10)),
        };
        let s = f.summary();
        assert!(s.contains("MissingSymbol"));
        assert!(s.contains("addTask"));
        assert!(s.contains("src/test.ts:3"));
    }

    #[test]
    fn test_serialization_roundtrip() {
        let failures = vec![
            TemplateFailure::MissingSymbol {
                symbol: "x".into(),
                available: vec![],
                suggestion: None,
                location: SourceLocation::new("test.ts", 1, None),
            },
            TemplateFailure::WrongArgCount {
                function: "f".into(),
                expected: 1,
                actual: 2,
                location: SourceLocation::new("test.ts", 2, Some(5)),
            },
            TemplateFailure::TypeMismatch {
                expected: "string".into(),
                actual: "number".into(),
                location: SourceLocation::new("test.ts", 3, Some(1)),
            },
            TemplateFailure::CompileError {
                code: "TS2339".into(),
                message: "not found".into(),
                location: SourceLocation::new("test.ts", 4, None),
            },
            TemplateFailure::AssertionFailure {
                test_name: "t".into(),
                expected: "a".into(),
                received: "b".into(),
                location: SourceLocation::new("test.ts", 5, Some(1)),
            },
            TemplateFailure::TestFailure {
                test_name: "t".into(),
                message: "timeout".into(),
                location: None,
            },
        ];

        for failure in &failures {
            let json = serde_json::to_string(failure).unwrap();
            let deserialized: TemplateFailure = serde_json::from_str(&json).unwrap();
            assert_eq!(*failure, deserialized);
        }
    }
}
