//! Shared validation types for use across all Rigorix modules.
//!
//! @canonical .pi/architecture/modules/common.md#validation
//!
//! Provides `ValidationResult<T>`, `ValidationError`, and `ValidationWarning`
//! types that can replace module-specific duplicates over time.

use serde::{Deserialize, Serialize};

/// A single validation error with a human-readable message and rule identifier.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ValidationError {
    /// Human-readable description of the validation failure.
    pub message: String,
    /// Machine-readable rule identifier (e.g., "max_parallelism_exceeded").
    pub rule: String,
    /// Optional reference to the specific node/field that failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node_id: Option<String>,
}

impl ValidationError {
    /// Create a new validation error.
    pub fn new(message: impl Into<String>, rule: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            rule: rule.into(),
            node_id: None,
        }
    }

    /// Create a validation error scoped to a specific node.
    pub fn for_node(
        message: impl Into<String>,
        rule: impl Into<String>,
        node_id: impl Into<String>,
    ) -> Self {
        Self {
            message: message.into(),
            rule: rule.into(),
            node_id: Some(node_id.into()),
        }
    }
}

/// A validation warning (non-fatal diagnostic).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ValidationWarning {
    /// Human-readable warning description.
    pub message: String,
    /// Machine-readable rule identifier.
    pub rule: String,
    /// Optional reference to the specific node/field.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node_id: Option<String>,
}

/// The result of a validation operation.
///
/// Can be in one of three states:
/// - `valid`: no errors (warnings may still be present)
/// - `invalid`: one or more errors (validation failed)
/// - `warnings_only`: passed with warnings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    /// Whether the validation passed (no errors).
    pub valid: bool,
    /// Validation errors (fatal).
    pub errors: Vec<ValidationError>,
    /// Validation warnings (non-fatal).
    pub warnings: Vec<ValidationWarning>,
}

impl ValidationResult {
    /// A valid result with no errors or warnings.
    pub fn valid() -> Self {
        Self {
            valid: true,
            errors: Vec::new(),
            warnings: Vec::new(),
        }
    }

    /// Create a result from lists of errors and warnings.
    pub fn from_lists(errors: Vec<ValidationError>, warnings: Vec<ValidationWarning>) -> Self {
        Self {
            valid: errors.is_empty(),
            errors,
            warnings,
        }
    }

    /// Add an error to this result.
    pub fn add_error(&mut self, error: ValidationError) {
        self.valid = false;
        self.errors.push(error);
    }

    /// Add a warning to this result.
    pub fn add_warning(&mut self, warning: ValidationWarning) {
        self.warnings.push(warning);
    }

    /// Returns `true` if there are no errors (warnings are acceptable).
    pub fn is_valid(&self) -> bool {
        self.valid
    }

    /// Returns `true` if there are errors.
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    /// Returns `true` if there are warnings.
    pub fn has_warnings(&self) -> bool {
        !self.warnings.is_empty()
    }

    /// Returns the number of errors.
    pub fn error_count(&self) -> usize {
        self.errors.len()
    }

    /// Returns the number of warnings.
    pub fn warning_count(&self) -> usize {
        self.warnings.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_result() {
        let result = ValidationResult::valid();
        assert!(result.is_valid());
        assert_eq!(result.error_count(), 0);
    }

    #[test]
    fn test_result_with_error() {
        let mut result = ValidationResult::valid();
        result.add_error(ValidationError::new("test error", "test_rule"));
        assert!(!result.is_valid());
        assert_eq!(result.error_count(), 1);
    }

    #[test]
    fn test_result_with_warning() {
        let mut result = ValidationResult::valid();
        result.add_warning(ValidationWarning {
            message: "test warning".to_string(),
            rule: "test_warn".to_string(),
            node_id: None,
        });
        assert!(result.is_valid());
        assert_eq!(result.warning_count(), 1);
    }

    #[test]
    fn test_error_for_node() {
        let err = ValidationError::for_node("node error", "node_rule", "node-1");
        assert_eq!(err.node_id, Some("node-1".to_string()));
    }

    #[test]
    fn test_from_lists() {
        let errors = vec![ValidationError::new("err1", "r1")];
        let warnings = vec![ValidationWarning {
            message: "warn1".to_string(),
            rule: "r2".to_string(),
            node_id: None,
        }];
        let result = ValidationResult::from_lists(errors, warnings);
        assert!(!result.is_valid());
        assert_eq!(result.error_count(), 1);
        assert_eq!(result.warning_count(), 1);
    }
}
