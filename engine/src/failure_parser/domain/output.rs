//! ParsedFailure — parsed result with all failures.
//!
//! @canonical .pi/architecture/modules/failure-parser.md#output
//! Implements: Contract Freeze — ParsedFailure, SourceContext structs
//! Issue: #495
//!
//! # Contract (Frozen)
//! - Output container for the complete parse result
//! - Carries the list of parsed failures and overall severity
//! - SourceContext provides symbol information for suggestion generation
//! - Serialization support for eventing and API responses

use serde::{Deserialize, Serialize};

use super::detail::{FailureDetail, FailureSeverity};

/// The complete result of parsing compiler/test output.
///
/// Returned by `FailureParserService::parse()`. Contains the list of
/// parsed failure details and an overall severity classification.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParsedFailure {
    /// Individual parsed failure details.
    pub failures: Vec<FailureDetail>,

    /// Overall severity of the entire failure set.
    pub overall_severity: FailureSeverity,

    /// Total number of failures found.
    pub total_count: usize,

    /// Number of fixable failures (where a suggestion was generated).
    pub fixable_count: usize,

    /// The source tool that produced the original output.
    pub source_tool: String,
}

impl ParsedFailure {
    /// Create a new ParsedFailure from a list of FailureDetail items.
    ///
    /// Automatically computes overall_severity, total_count, and fixable_count.
    pub fn from_failures(failures: Vec<FailureDetail>, source_tool: impl Into<String>) -> Self {
        let total_count = failures.len();
        let fixable_count = failures
            .iter()
            .filter(|d| d.suggested_fix.is_some())
            .count();
        let overall_severity = Self::compute_overall_severity(&failures);

        Self {
            failures,
            overall_severity,
            total_count,
            fixable_count,
            source_tool: source_tool.into(),
        }
    }

    /// Returns `true` if there are no failures.
    pub fn is_clean(&self) -> bool {
        self.failures.is_empty()
    }

    /// Returns true if all failures have suggested fixes.
    pub fn all_fixable(&self) -> bool {
        self.fixable_count == self.total_count && self.total_count > 0
    }

    /// Compute the overall severity from a list of failure details.
    ///
    /// CompileBlock > TestBlock > Warning.
    fn compute_overall_severity(failures: &[FailureDetail]) -> FailureSeverity {
        for detail in failures {
            if detail.severity == FailureSeverity::CompileBlock {
                return FailureSeverity::CompileBlock;
            }
        }
        for detail in failures {
            if detail.severity == FailureSeverity::TestBlock {
                return FailureSeverity::TestBlock;
            }
        }
        if failures.is_empty() {
            FailureSeverity::Warning
        } else {
            FailureSeverity::Warning
        }
    }
}

/// Context about the source code available for suggestion generation.
///
/// Provided to parsers so they can cross-reference error symbols
/// against the actual source code to generate meaningful fix suggestions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SourceContext {
    /// Available symbols keyed by file path (relative to project root).
    /// The map value is a list of symbol names (function names, method names, type names).
    pub symbols_by_file: std::collections::HashMap<String, Vec<String>>,

    /// Full source content keyed by file path, for context-aware suggestions.
    /// Limited to files in the immediate error scope for performance.
    pub source_by_file: std::collections::HashMap<String, String>,
}

impl SourceContext {
    /// Create a new empty SourceContext.
    pub fn empty() -> Self {
        Self {
            symbols_by_file: std::collections::HashMap::new(),
            source_by_file: std::collections::HashMap::new(),
        }
    }

    /// Get symbols available in a specific file.
    pub fn symbols_in_file(&self, file: &str) -> Vec<String> {
        self.symbols_by_file
            .get(file)
            .cloned()
            .unwrap_or_default()
    }

    /// Get source content for a specific file.
    pub fn source_for_file(&self, file: &str) -> Option<&str> {
        self.source_by_file.get(file).map(|s| s.as_str())
    }

    /// Returns `true` if this context has any symbols or source content.
    pub fn has_content(&self) -> bool {
        !self.symbols_by_file.is_empty() || !self.source_by_file.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::failure_parser::domain::failure::{SourceLocation, TemplateFailure};

    fn make_detail(suggestion: Option<&str>, severity: FailureSeverity) -> FailureDetail {
        FailureDetail::new(
            TemplateFailure::MissingSymbol {
                symbol: "x".into(),
                available: vec![],
                suggestion: None,
                location: SourceLocation::new("test.ts", 1, None),
            },
            suggestion.map(|s| s.to_string()),
            severity,
            "error".into(),
            "tsc",
            1.0,
        )
    }

    #[test]
    fn test_parsed_failure_empty() {
        let parsed = ParsedFailure::from_failures(vec![], "tsc");
        assert!(parsed.is_clean());
        assert_eq!(parsed.total_count, 0);
        assert_eq!(parsed.fixable_count, 0);
    }

    #[test]
    fn test_parsed_failure_with_failures() {
        let failures = vec![
            make_detail(Some("fix 1"), FailureSeverity::CompileBlock),
            make_detail(None, FailureSeverity::TestBlock),
            make_detail(Some("fix 2"), FailureSeverity::Warning),
        ];
        let parsed = ParsedFailure::from_failures(failures, "tsc");
        assert!(!parsed.is_clean());
        assert_eq!(parsed.total_count, 3);
        assert_eq!(parsed.fixable_count, 2);
        assert_eq!(parsed.overall_severity, FailureSeverity::CompileBlock);
    }

    #[test]
    fn test_parsed_failure_overall_severity_compile_block_wins() {
        let failures = vec![
            make_detail(None, FailureSeverity::Warning),
            make_detail(None, FailureSeverity::CompileBlock),
            make_detail(None, FailureSeverity::TestBlock),
        ];
        let parsed = ParsedFailure::from_failures(failures, "tsc");
        assert_eq!(parsed.overall_severity, FailureSeverity::CompileBlock);
    }

    #[test]
    fn test_parsed_failure_overall_severity_test_block() {
        let failures = vec![
            make_detail(None, FailureSeverity::Warning),
            make_detail(None, FailureSeverity::TestBlock),
        ];
        let parsed = ParsedFailure::from_failures(failures, "jest");
        assert_eq!(parsed.overall_severity, FailureSeverity::TestBlock);
    }

    #[test]
    fn test_source_context_empty() {
        let ctx = SourceContext::empty();
        assert!(!ctx.has_content());
        assert!(ctx.symbols_in_file("test.ts").is_empty());
    }

    #[test]
    fn test_source_context_with_symbols() {
        let mut ctx = SourceContext::empty();
        ctx.symbols_by_file
            .insert("test.ts".into(), vec!["add".into(), "remove".into()]);
        ctx.source_by_file
            .insert("test.ts".into(), "content".into());

        assert!(ctx.has_content());
        assert_eq!(ctx.symbols_in_file("test.ts"), vec!["add", "remove"]);
        assert_eq!(ctx.source_for_file("test.ts"), Some("content"));
    }

    #[test]
    fn test_all_fixable() {
        let failures = vec![
            make_detail(Some("fix"), FailureSeverity::CompileBlock),
            make_detail(Some("fix"), FailureSeverity::TestBlock),
        ];
        let parsed = ParsedFailure::from_failures(failures, "tsc");
        assert!(parsed.all_fixable());
    }

    #[test]
    fn test_not_all_fixable() {
        let failures = vec![
            make_detail(Some("fix"), FailureSeverity::CompileBlock),
            make_detail(None, FailureSeverity::TestBlock),
        ];
        let parsed = ParsedFailure::from_failures(failures, "tsc");
        assert!(!parsed.all_fixable());
    }
}
