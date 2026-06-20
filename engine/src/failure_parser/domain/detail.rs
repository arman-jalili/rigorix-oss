//! FailureDetail — individual error with location and suggestion.
//!
//! @canonical .pi/architecture/modules/failure-parser.md#detail
//! Implements: Contract Freeze — FailureDetail struct
//! Issue: #495
//!
//! # Contract (Frozen)
//! - Pairs a TemplateFailure with a structured FailureDetail
//! - Carries the suggested fix, severity, and classification metadata
//! - Serialization support for eventing and API responses

use serde::{Deserialize, Serialize};

use super::failure::{SourceLocation, TemplateFailure};

/// Individual parsed failure with full context.
///
/// Wraps a `TemplateFailure` with the suggested fix, severity
/// classification, and raw error text for traceability.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FailureDetail {
    /// The typed failure classification.
    pub failure: TemplateFailure,

    /// Human-readable suggested fix, if one could be determined.
    /// This is the actionable guidance for LLM self-correction.
    pub suggested_fix: Option<String>,

    /// Severity classification of this failure.
    pub severity: FailureSeverity,

    /// The raw error line from the compiler/test output.
    pub raw_line: String,

    /// The source tool that produced this error (e.g., "tsc", "jest", "rustc").
    pub source_tool: String,

    /// Confidence that this parsing is correct (0.0–1.0).
    pub confidence: f64,
}

impl FailureDetail {
    /// Create a new FailureDetail.
    pub fn new(
        failure: TemplateFailure,
        suggested_fix: Option<String>,
        severity: FailureSeverity,
        raw_line: String,
        source_tool: impl Into<String>,
        confidence: f64,
    ) -> Self {
        Self {
            failure,
            suggested_fix,
            severity,
            raw_line,
            source_tool: source_tool.into(),
            confidence,
        }
    }

    /// Returns the source location from the underlying failure, if available.
    pub fn location(&self) -> Option<SourceLocation> {
        match &self.failure {
            TemplateFailure::MissingSymbol { location, .. } => Some(location.clone()),
            TemplateFailure::WrongArgCount { location, .. } => Some(location.clone()),
            TemplateFailure::TypeMismatch { location, .. } => Some(location.clone()),
            TemplateFailure::CompileError { location, .. } => Some(location.clone()),
            TemplateFailure::AssertionFailure { location, .. } => Some(location.clone()),
            TemplateFailure::TestFailure { location, .. } => location.clone(),
        }
    }

    /// Returns a compact one-line representation for logging.
    pub fn to_log_line(&self) -> String {
        let loc = self.location();
        let loc_str = loc.as_ref().map(|l| l.to_compact()).unwrap_or_default();
        let fix = self.suggested_fix.as_deref().unwrap_or("no suggestion");
        format!(
            "[{}] {} (severity={:?}, confidence={:.2}) — fix: {}",
            self.source_tool, loc_str, self.severity, self.confidence, fix
        )
    }
}

/// Severity classification for a parsed failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FailureSeverity {
    /// Compile errors — template is syntactically invalid.
    CompileBlock,
    /// Test failures — template compiled but logic is wrong.
    TestBlock,
    /// Warnings — template works but has issues.
    Warning,
}

impl FailureSeverity {
    /// Returns the canonical snake_case name.
    pub fn as_str(&self) -> &'static str {
        match self {
            FailureSeverity::CompileBlock => "compile_block",
            FailureSeverity::TestBlock => "test_block",
            FailureSeverity::Warning => "warning",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_failure_detail_new() {
        let failure = TemplateFailure::MissingSymbol {
            symbol: "addTask".into(),
            available: vec!["add".into()],
            suggestion: Some("Use 'add'".into()),
            location: SourceLocation::new("test.ts", 3, Some(10)),
        };
        let detail = FailureDetail::new(
            failure.clone(),
            Some("Use 'add' instead".into()),
            FailureSeverity::CompileBlock,
            "error TS2339: Property 'addTask' does not exist".into(),
            "tsc",
            0.95,
        );
        assert_eq!(detail.failure, failure);
        assert_eq!(detail.source_tool, "tsc");
        assert_eq!(detail.severity, FailureSeverity::CompileBlock);
    }

    #[test]
    fn test_failure_detail_location() {
        let failure = TemplateFailure::MissingSymbol {
            symbol: "x".into(),
            available: vec![],
            suggestion: None,
            location: SourceLocation::new("test.ts", 1, None),
        };
        let detail = FailureDetail::new(
            failure,
            None,
            FailureSeverity::CompileBlock,
            "error".into(),
            "tsc",
            1.0,
        );
        assert_eq!(
            detail.location(),
            Some(SourceLocation::new("test.ts", 1, None))
        );
    }

    #[test]
    fn test_failure_detail_test_failure_location() {
        let failure = TemplateFailure::TestFailure {
            test_name: "test".into(),
            message: "failed".into(),
            location: None,
        };
        let detail = FailureDetail::new(
            failure,
            None,
            FailureSeverity::TestBlock,
            "FAIL".into(),
            "jest",
            1.0,
        );
        assert_eq!(detail.location(), None);
    }

    #[test]
    fn test_severity_as_str() {
        assert_eq!(FailureSeverity::CompileBlock.as_str(), "compile_block");
        assert_eq!(FailureSeverity::TestBlock.as_str(), "test_block");
        assert_eq!(FailureSeverity::Warning.as_str(), "warning");
    }
}
