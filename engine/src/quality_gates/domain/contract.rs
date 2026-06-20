//! GreenContract — declares required quality level and evaluates observed level.
//!
//! @canonical .pi/architecture/modules/quality-gates.md#greencontract
//! Implements: Contract Freeze — GreenContract struct
//! Issue: #449 (quality-gates epic)
//!
//! # Contract (Frozen)
//! - A `GreenContract` is an immutable value object once constructed
//! - `required_level` specifies the minimum `QualityLevel` required
//! - `evaluate()` compares observed level against required level
//! - Implements `Clone`, `Copy`, `Debug`, `PartialEq`, `Eq` for testability
//! - Serialization support for configuration and API responses

use serde::{Deserialize, Serialize};

use super::level::QualityLevel;
use super::outcome::QualityGateOutcome;

/// Declares a required quality level and evaluates an observed level against it.
///
/// A `GreenContract` is the binding agreement between a task's quality
/// requirements and the actual test scope that was executed. The orchestrator
/// uses it to decide whether a task has sufficient quality evidence to proceed
/// with closeout.
///
/// # Examples
///
/// ```
/// use rigorix_engine::quality_gates::domain::{GreenContract, QualityLevel, QualityGateOutcome};
///
/// let contract = GreenContract::new(QualityLevel::Workspace);
/// let outcome = contract.evaluate(Some(QualityLevel::Package));
/// assert!(matches!(outcome, QualityGateOutcome::Unsatisfied { .. }));
///
/// let outcome = contract.evaluate(Some(QualityLevel::Workspace));
/// assert!(matches!(outcome, QualityGateOutcome::Satisfied { .. }));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct GreenContract {
    /// The minimum quality level required for this contract to be satisfied.
    pub required_level: QualityLevel,
}

impl GreenContract {
    /// Create a new `GreenContract` with the required quality level.
    pub fn new(required_level: QualityLevel) -> Self {
        Self { required_level }
    }

    /// Evaluate an observed quality level against this contract.
    ///
    /// Returns:
    /// - `QualityGateOutcome::Satisfied` if the observed level meets or exceeds
    ///   the required level.
    /// - `QualityGateOutcome::Unsatisfied` if the observed level is below
    ///   the required level, or if `observed` is `None`.
    ///
    /// # Semantics
    ///
    /// | Observed | Required | Outcome |
    /// |----------|----------|---------|
    /// | Workspace | Package | Satisfied (Workspace >= Package) |
    /// | Package | Workspace | Unsatisfied (Package < Workspace, gap=1) |
    /// | None | Workspace | Unsatisfied (falls back to TargetedTests, gap=2) |
    pub fn evaluate(&self, observed: Option<QualityLevel>) -> QualityGateOutcome {
        match observed {
            Some(level) if level >= self.required_level => QualityGateOutcome::Satisfied {
                required: self.required_level,
                observed: level,
            },
            Some(level) => QualityGateOutcome::Unsatisfied {
                required: self.required_level,
                observed: level,
                gap: self.required_level as i32 - level as i32,
            },
            None => QualityGateOutcome::Unsatisfied {
                required: self.required_level,
                observed: QualityLevel::TargetedTests,
                gap: self.required_level as i32,
            },
        }
    }

    /// Returns a human-readable description of this contract.
    pub fn description(&self) -> String {
        format!(
            "Requires at least {} quality level",
            self.required_level.as_str()
        )
    }
}

impl Default for GreenContract {
    /// Default contract requires at least `Package` level.
    fn default() -> Self {
        Self {
            required_level: QualityLevel::Package,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_contract() {
        let contract = GreenContract::new(QualityLevel::Workspace);
        assert_eq!(contract.required_level, QualityLevel::Workspace);
    }

    #[test]
    fn test_default_contract() {
        let contract = GreenContract::default();
        assert_eq!(contract.required_level, QualityLevel::Package);
    }

    #[test]
    fn test_evaluate_satisfied_equal() {
        let contract = GreenContract::new(QualityLevel::Package);
        let outcome = contract.evaluate(Some(QualityLevel::Package));
        assert!(matches!(outcome, QualityGateOutcome::Satisfied { .. }));
    }

    #[test]
    fn test_evaluate_satisfied_higher() {
        let contract = GreenContract::new(QualityLevel::Package);
        let outcome = contract.evaluate(Some(QualityLevel::Workspace));
        assert!(matches!(outcome, QualityGateOutcome::Satisfied { .. }));
    }

    #[test]
    fn test_evaluate_unsatisfied_lower() {
        let contract = GreenContract::new(QualityLevel::Workspace);
        let outcome = contract.evaluate(Some(QualityLevel::Package));
        assert!(matches!(outcome, QualityGateOutcome::Unsatisfied { .. }));
    }

    #[test]
    fn test_evaluate_unsatisfied_gap() {
        let contract = GreenContract::new(QualityLevel::Workspace);
        if let QualityGateOutcome::Unsatisfied { gap, .. } = contract.evaluate(Some(QualityLevel::TargetedTests)) {
            assert_eq!(gap, 2);
        } else {
            panic!("Expected Unsatisfied");
        }
    }

    #[test]
    fn test_evaluate_none_returns_unsatisfied() {
        let contract = GreenContract::new(QualityLevel::Workspace);
        let outcome = contract.evaluate(None);
        assert!(matches!(outcome, QualityGateOutcome::Unsatisfied { .. }));
        if let QualityGateOutcome::Unsatisfied { gap, observed, .. } = outcome {
            assert_eq!(observed, QualityLevel::TargetedTests);
            assert_eq!(gap, 2);
        }
    }

    #[test]
    fn test_evaluate_merge_ready_satisfied() {
        let contract = GreenContract::new(QualityLevel::MergeReady);
        let outcome = contract.evaluate(Some(QualityLevel::MergeReady));
        assert!(matches!(outcome, QualityGateOutcome::Satisfied { .. }));
    }

    #[test]
    fn test_evaluate_merge_ready_unsatisfied() {
        let contract = GreenContract::new(QualityLevel::MergeReady);
        let outcome = contract.evaluate(Some(QualityLevel::Workspace));
        assert!(matches!(outcome, QualityGateOutcome::Unsatisfied { gap: 1, .. }));
    }

    #[test]
    fn test_description() {
        let contract = GreenContract::new(QualityLevel::MergeReady);
        assert!(contract.description().contains("merge_ready"));
    }

    #[test]
    fn test_serialization_roundtrip() {
        let contract = GreenContract::new(QualityLevel::Workspace);
        let json = serde_json::to_string(&contract).unwrap();
        let deserialized: GreenContract = serde_json::from_str(&json).unwrap();
        assert_eq!(contract, deserialized);
    }
}
