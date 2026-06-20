//! SyntaxGateResult — outcome of post-edit tree-sitter syntax verification.
//!
//! @canonical .pi/architecture/modules/code-generation.md#syntax-result
//! Implements: Contract Freeze — SyntaxGateResult, SyntaxError
//! Issue: #424
//!
//! Defines the outcome of running tree-sitter AST validation on a file
//! after an edit. The syntax gate is optional — it can be configured to
//! block edits on syntax errors or merely warn.
//!
//! # Contract (Frozen)
//! - Three variants: Passed, Failed (with errors), Skipped (no parser)
//! - SyntaxError carries structured location context for the LLM
//! - All types are serializable for API responses

use serde::{Deserialize, Serialize};

/// Outcome of post-edit tree-sitter syntax verification.
///
/// # Variants
///
/// | Variant | Meaning | Action |
/// |---------|---------|--------|
/// | `Passed` | File parses without errors | Edit confirmed |
/// | `Failed` | Syntax errors detected | Edit applied but errors reported to LLM |
/// | `Skipped` | No parser available for language | Edit applied, no verification |
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyntaxGateResult {
    /// File parses without errors.
    Passed,

    /// Syntax errors were found — returned with error locations.
    Failed {
        /// The syntax errors detected.
        errors: Vec<SyntaxError>,
    },

    /// No parser available for this language (not an error).
    Skipped {
        /// Reason the syntax check was skipped.
        reason: String,
    },
}

/// A single syntax error detected by tree-sitter.
///
/// Provides structured location context so the LLM can understand
/// exactly what went wrong and issue a corrective edit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SyntaxError {
    /// Line number where the error occurred (1-indexed).
    pub line: usize,

    /// Column number where the error occurred (1-indexed).
    pub column: usize,

    /// Human-readable error message.
    pub message: String,

    /// Surrounding code context for the LLM to understand the error.
    pub context: String,
}

impl SyntaxGateResult {
    /// Returns true if the syntax check passed or was skipped.
    pub fn is_success(&self) -> bool {
        matches!(
            self,
            SyntaxGateResult::Passed | SyntaxGateResult::Skipped { .. }
        )
    }

    /// Returns true if the syntax check failed.
    pub fn is_failed(&self) -> bool {
        matches!(self, SyntaxGateResult::Failed { .. })
    }

    /// Returns the syntax errors if the check failed.
    pub fn errors(&self) -> Vec<&SyntaxError> {
        match self {
            SyntaxGateResult::Failed { errors } => errors.iter().collect(),
            _ => vec![],
        }
    }

    /// Returns the skip reason if skipped.
    pub fn skip_reason(&self) -> Option<&str> {
        match self {
            SyntaxGateResult::Skipped { reason } => Some(reason),
            _ => None,
        }
    }

    /// Create a Passed result.
    pub fn passed() -> Self {
        SyntaxGateResult::Passed
    }

    /// Create a Failed result with errors.
    pub fn failed(errors: Vec<SyntaxError>) -> Self {
        SyntaxGateResult::Failed { errors }
    }

    /// Create a Skipped result.
    pub fn skipped(reason: impl Into<String>) -> Self {
        SyntaxGateResult::Skipped {
            reason: reason.into(),
        }
    }
}

impl SyntaxError {
    /// Create a new syntax error.
    pub fn new(
        line: usize,
        column: usize,
        message: impl Into<String>,
        context: impl Into<String>,
    ) -> Self {
        Self {
            line,
            column,
            message: message.into(),
            context: context.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_passed() {
        let result = SyntaxGateResult::passed();
        assert!(result.is_success());
        assert!(!result.is_failed());
        assert!(result.errors().is_empty());
        assert!(result.skip_reason().is_none());
    }

    #[test]
    fn test_failed() {
        let errors = vec![SyntaxError::new(42, 5, "expected `;`", "let x = 5")];
        let result = SyntaxGateResult::failed(errors);
        assert!(!result.is_success());
        assert!(result.is_failed());
        assert_eq!(result.errors().len(), 1);
        assert_eq!(result.errors()[0].line, 42);
        assert_eq!(result.errors()[0].column, 5);
        assert_eq!(result.errors()[0].message, "expected `;`");
    }

    #[test]
    fn test_skipped() {
        let result = SyntaxGateResult::skipped("no parser for markdown");
        assert!(result.is_success());
        assert!(!result.is_failed());
        assert_eq!(result.skip_reason(), Some("no parser for markdown"));
    }

    #[test]
    fn test_syntax_error_construction() {
        let err = SyntaxError::new(1, 1, "unexpected token", "fn main() {");
        assert_eq!(err.line, 1);
        assert_eq!(err.column, 1);
        assert_eq!(err.message, "unexpected token");
        assert_eq!(err.context, "fn main() {");
    }

    #[test]
    fn test_serde_roundtrip_passed() {
        let result = SyntaxGateResult::passed();
        let json = serde_json::to_string(&result).unwrap();
        let deserialized: SyntaxGateResult = serde_json::from_str(&json).unwrap();
        assert_eq!(result, deserialized);
    }

    #[test]
    fn test_serde_roundtrip_failed() {
        let result =
            SyntaxGateResult::failed(vec![SyntaxError::new(10, 3, "expected `)`", "if (true")]);
        let json = serde_json::to_string(&result).unwrap();
        let deserialized: SyntaxGateResult = serde_json::from_str(&json).unwrap();
        assert!(deserialized.is_failed());
    }

    #[test]
    fn test_serde_roundtrip_skipped() {
        let result = SyntaxGateResult::skipped("no parser");
        let json = serde_json::to_string(&result).unwrap();
        let deserialized: SyntaxGateResult = serde_json::from_str(&json).unwrap();
        assert_eq!(result, deserialized);
    }

    #[test]
    fn test_syntax_error_serde() {
        let err = SyntaxError::new(15, 8, "expected `;`", "let x = 5");
        let json = serde_json::to_string(&err).unwrap();
        let deserialized: SyntaxError = serde_json::from_str(&json).unwrap();
        assert_eq!(err, deserialized);
    }
}
