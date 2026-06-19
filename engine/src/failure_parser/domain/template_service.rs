//! TemplateFailureService — domain service for TemplateFailure operations.
//!
//! @canonical .pi/architecture/modules/failure-parser.md#failure
//! Implements: TemplateFailureService — utility operations on TemplateFailure collections
//! Issue: #496
//!
//! Provides operations on collections of TemplateFailure instances:
//! grouping, filtering, summarizing, and severity analysis.

use crate::failure_parser::domain::{
    detail::FailureSeverity,
    failure::TemplateFailure,
    output::ParsedFailure,
};

/// Domain service for TemplateFailure operations.
///
/// Provides utility functions for working with TemplateFailure collections.
/// This is stateless — all operations are pure functions over data.
pub struct TemplateFailureService;

impl TemplateFailureService {
    /// Group failures by their variant type.
    ///
    /// Returns a map of variant name → list of failures.
    pub fn group_by_variant(failures: &[TemplateFailure]) -> std::collections::HashMap<&'static str, Vec<&TemplateFailure>> {
        let mut groups: std::collections::HashMap<&'static str, Vec<&TemplateFailure>> = std::collections::HashMap::new();
        for f in failures {
            groups.entry(f.variant_name()).or_default().push(f);
        }
        groups
    }

    /// Filter failures that have suggested fixes (is_fixable() returns true).
    pub fn fixable(failures: &[TemplateFailure]) -> Vec<&TemplateFailure> {
        failures.iter().filter(|f| f.is_fixable()).collect()
    }

    /// Filter failures that are NOT fixable.
    pub fn non_fixable(failures: &[TemplateFailure]) -> Vec<&TemplateFailure> {
        failures.iter().filter(|f| !f.is_fixable()).collect()
    }

    /// Compute overall severity from a list of failures.
    ///
    /// CompileBlock > TestBlock > Warning.
    pub fn classify_severity(failures: &[TemplateFailure]) -> FailureSeverity {
        // Check if any failure has a compile/location that suggests compile block
        // Heuristic: CompileError, MissingSymbol, WrongArgCount, TypeMismatch are compile blocks
        for f in failures {
            match f {
                TemplateFailure::CompileError { .. }
                | TemplateFailure::MissingSymbol { .. }
                | TemplateFailure::WrongArgCount { .. }
                | TemplateFailure::TypeMismatch { .. } => return FailureSeverity::CompileBlock,
                _ => {}
            }
        }
        // Test failures
        for f in failures {
            match f {
                TemplateFailure::AssertionFailure { .. }
                | TemplateFailure::TestFailure { .. } => return FailureSeverity::TestBlock,
                _ => {}
            }
        }
        FailureSeverity::Warning
    }

    /// Generate a summary string for a list of failures.
    ///
    /// Returns a string like:
    /// "FAILURE ANALYSIS: 3 errors found (2 fixable, 1 non-fixable)"
    pub fn summary(failures: &[TemplateFailure]) -> String {
        let total = failures.len();
        let fixable = Self::fixable(failures).len();
        let non_fixable = total - fixable;
        let severity = Self::classify_severity(failures);

        format!(
            "FAILURE ANALYSIS: {} {} found ({} fixable, {} non-fixable, severity: {:?})",
            total,
            if total == 1 { "error" } else { "errors" },
            fixable,
            non_fixable,
            severity,
        )
    }

    /// Get unique error codes from CompileError failures.
    pub fn error_codes(failures: &[TemplateFailure]) -> std::collections::BTreeSet<String> {
        failures
            .iter()
            .filter_map(|f| {
                if let TemplateFailure::CompileError { code, .. } = f {
                    Some(code.clone())
                } else {
                    None
                }
            })
            .collect()
    }

    /// Get unique test names from test failures.
    pub fn test_names(failures: &[TemplateFailure]) -> std::collections::BTreeSet<String> {
        failures
            .iter()
            .filter_map(|f| match f {
                TemplateFailure::AssertionFailure { test_name, .. } => Some(test_name.clone()),
                TemplateFailure::TestFailure { test_name, .. } => Some(test_name.clone()),
                _ => None,
            })
            .collect()
    }

    /// Create a ParsedFailure from a list of TemplateFailures and a source tool name.
    pub fn to_parsed(failures: Vec<TemplateFailure>, source_tool: impl Into<String>) -> ParsedFailure {
        let tool_str: String = source_tool.into();
        let details: Vec<_> = failures
            .into_iter()
            .map(|f| {
                let severity = match &f {
                    TemplateFailure::CompileError { .. }
                    | TemplateFailure::MissingSymbol { .. }
                    | TemplateFailure::WrongArgCount { .. }
                    | TemplateFailure::TypeMismatch { .. } => FailureSeverity::CompileBlock,
                    TemplateFailure::AssertionFailure { .. }
                    | TemplateFailure::TestFailure { .. } => FailureSeverity::TestBlock,
                };
                let raw = f.summary();
                crate::failure_parser::domain::detail::FailureDetail::new(
                    f,
                    None,
                    severity,
                    raw,
                    tool_str.clone(),
                    1.0,
                )
            })
            .collect();

        ParsedFailure::from_failures(details, tool_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::failure_parser::domain::failure::SourceLocation;

    fn make_missing_symbol(symbol: &str) -> TemplateFailure {
        TemplateFailure::MissingSymbol {
            symbol: symbol.to_string(),
            available: vec!["add".into(), "remove".into()],
            suggestion: Some(format!("Use 'add' instead of '{}'", symbol)),
            location: SourceLocation::new("test.ts", 3, Some(10)),
        }
    }

    fn make_compile_error(code: &str) -> TemplateFailure {
        TemplateFailure::CompileError {
            code: code.to_string(),
            message: "Some compile error".into(),
            location: SourceLocation::new("test.ts", 5, None),
        }
    }

    fn make_test_failure(name: &str) -> TemplateFailure {
        TemplateFailure::TestFailure {
            test_name: name.to_string(),
            message: "Assertion failed".into(),
            location: None,
        }
    }

    fn make_assertion_failure(name: &str) -> TemplateFailure {
        TemplateFailure::AssertionFailure {
            test_name: name.to_string(),
            expected: "true".into(),
            received: "false".into(),
            location: SourceLocation::new("test.ts", 10, Some(1)),
        }
    }

    fn make_type_mismatch() -> TemplateFailure {
        TemplateFailure::TypeMismatch {
            expected: "string".into(),
            actual: "number".into(),
            location: SourceLocation::new("test.ts", 15, None),
        }
    }

    fn make_wrong_arg_count() -> TemplateFailure {
        TemplateFailure::WrongArgCount {
            function: "add".into(),
            expected: 2,
            actual: 3,
            location: SourceLocation::new("test.ts", 20, None),
        }
    }

    #[test]
    fn test_group_by_variant() {
        let failures = vec![
            make_missing_symbol("addTask"),
            make_compile_error("TS2339"),
            make_test_failure("test1"),
            make_missing_symbol("removeItem"),
        ];
        let groups = TemplateFailureService::group_by_variant(&failures);
        assert_eq!(groups.len(), 3);
        assert_eq!(groups.get("missing_symbol").unwrap().len(), 2);
        assert_eq!(groups.get("compile_error").unwrap().len(), 1);
        assert_eq!(groups.get("test_failure").unwrap().len(), 1);
    }

    #[test]
    fn test_fixable_filter() {
        let failures = vec![
            make_missing_symbol("x"),
            make_compile_error("E001"),
            make_test_failure("t1"),
        ];
        let fixable = TemplateFailureService::fixable(&failures);
        assert_eq!(fixable.len(), 1);
        assert_eq!(fixable[0].variant_name(), "missing_symbol");
    }

    #[test]
    fn test_non_fixable_filter() {
        let failures = vec![
            make_missing_symbol("x"),
            make_compile_error("E001"),
            make_test_failure("t1"),
        ];
        let non_fixable = TemplateFailureService::non_fixable(&failures);
        assert_eq!(non_fixable.len(), 2);
    }

    #[test]
    fn test_classify_severity_compile_block_wins() {
        let failures = vec![
            make_test_failure("t1"),
            make_compile_error("E001"),
        ];
        assert_eq!(
            TemplateFailureService::classify_severity(&failures),
            FailureSeverity::CompileBlock
        );
    }

    #[test]
    fn test_classify_severity_test_block() {
        let failures = vec![
            make_test_failure("t1"),
            make_assertion_failure("t2"),
        ];
        assert_eq!(
            TemplateFailureService::classify_severity(&failures),
            FailureSeverity::TestBlock
        );
    }

    #[test]
    fn test_classify_severity_warning() {
        // No failures means warning
        let failures: Vec<TemplateFailure> = vec![];
        assert_eq!(
            TemplateFailureService::classify_severity(&failures),
            FailureSeverity::Warning
        );
    }

    #[test]
    fn test_classify_severity_compile_types() {
        let failures = vec![
            make_type_mismatch(),
            make_wrong_arg_count(),
        ];
        assert_eq!(
            TemplateFailureService::classify_severity(&failures),
            FailureSeverity::CompileBlock
        );
    }

    #[test]
    fn test_summary_single() {
        let failures = vec![make_missing_symbol("x")];
        let s = TemplateFailureService::summary(&failures);
        assert!(s.contains("1 error"));
        assert!(s.contains("1 fixable"));
    }

    #[test]
    fn test_summary_multiple() {
        let failures = vec![
            make_missing_symbol("x"),
            make_compile_error("E001"),
            make_test_failure("t1"),
        ];
        let s = TemplateFailureService::summary(&failures);
        assert!(s.contains("3 errors"));
        assert!(s.contains("1 fixable"));
    }

    #[test]
    fn test_summary_empty() {
        let failures: Vec<TemplateFailure> = vec![];
        let s = TemplateFailureService::summary(&failures);
        assert!(s.contains("0 errors"));
    }

    #[test]
    fn test_error_codes() {
        let failures = vec![
            make_compile_error("TS2339"),
            make_compile_error("TS2554"),
            make_missing_symbol("x"),
            make_compile_error("TS2339"),
        ];
        let codes = TemplateFailureService::error_codes(&failures);
        assert_eq!(codes.len(), 2);
        assert!(codes.contains("TS2339"));
        assert!(codes.contains("TS2554"));
    }

    #[test]
    fn test_test_names() {
        let failures = vec![
            make_test_failure("should_add_task"),
            make_assertion_failure("should_remove_task"),
            make_compile_error("E001"),
        ];
        let names = TemplateFailureService::test_names(&failures);
        assert_eq!(names.len(), 2);
        assert!(names.contains("should_add_task"));
        assert!(names.contains("should_remove_task"));
    }

    #[test]
    fn test_to_parsed() {
        let failures = vec![
            make_missing_symbol("x"),
            make_test_failure("t1"),
        ];
        let parsed = TemplateFailureService::to_parsed(failures, "tsc");
        assert_eq!(parsed.total_count, 2);
        assert_eq!(parsed.source_tool, "tsc");
        assert!(!parsed.is_clean());
    }

    #[test]
    fn test_to_parsed_empty() {
        let parsed = TemplateFailureService::to_parsed(vec![], "jest");
        assert!(parsed.is_clean());
        assert_eq!(parsed.total_count, 0);
    }

    #[test]
    fn test_group_by_variant_empty() {
        let groups = TemplateFailureService::group_by_variant(&[]);
        assert!(groups.is_empty());
    }

    #[test]
    fn test_error_codes_empty() {
        let codes = TemplateFailureService::error_codes(&[]);
        assert!(codes.is_empty());
    }
}
