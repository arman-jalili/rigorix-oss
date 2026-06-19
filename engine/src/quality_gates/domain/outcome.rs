//! QualityGateOutcome — result of evaluating a GreenContract.
//!
//! @canonical .pi/architecture/modules/quality-gates.md#qualitygateoutcome
//! Implements: Contract Freeze — QualityGateOutcome enum
//! Issue: #449 (quality-gates epic)
//!
//! # Contract (Frozen)
//! - Two mutually exclusive outcomes: Satisfied or Unsatisfied
//! - Both variants carry `required` and `observed` QualityLevel for diagnostics
//! - Unsatisfied carries `gap` (number of levels below requirement)
//! - Uses `#[serde(tag = "outcome")]` for tagged JSON serialization
//! - Implements `Clone`, `Debug`, `PartialEq`, `Eq` for testability

use serde::{Deserialize, Serialize};

use super::level::QualityLevel;

/// Result of evaluating a `GreenContract` against an observed `QualityLevel`.
///
/// The orchestrator uses this outcome to decide whether a task has sufficient
/// quality evidence to proceed with closeout, or whether broader testing is
/// required.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum QualityGateOutcome {
    /// The observed quality level meets or exceeds the required level.
    /// The gate is passed — the task can proceed.
    Satisfied {
        /// The minimum quality level required by the contract.
        required: QualityLevel,
        /// The quality level that was actually observed.
        observed: QualityLevel,
    },

    /// The observed quality level is below the required level.
    /// The gate is not passed — broader testing is required, or
    /// escalation should be triggered.
    Unsatisfied {
        /// The minimum quality level required by the contract.
        required: QualityLevel,
        /// The quality level that was actually observed.
        observed: QualityLevel,
        /// How many levels below the requirement (positive integer).
        /// E.g., gap=2 means observed is 2 levels below required.
        gap: i32,
    },
}

impl QualityGateOutcome {
    /// Returns `true` if this outcome is satisfied.
    pub fn is_satisfied(&self) -> bool {
        matches!(self, QualityGateOutcome::Satisfied { .. })
    }

    /// Returns `true` if this outcome is unsatisfied.
    pub fn is_unsatisfied(&self) -> bool {
        matches!(self, QualityGateOutcome::Unsatisfied { .. })
    }

    /// Returns the required quality level from the outcome.
    pub fn required_level(&self) -> QualityLevel {
        match self {
            QualityGateOutcome::Satisfied { required, .. }
            | QualityGateOutcome::Unsatisfied { required, .. } => *required,
        }
    }

    /// Returns the observed quality level from the outcome.
    pub fn observed_level(&self) -> QualityLevel {
        match self {
            QualityGateOutcome::Satisfied { observed, .. }
            | QualityGateOutcome::Unsatisfied { observed, .. } => *observed,
        }
    }

    /// Returns the gap if unsatisfied, or `None` if satisfied.
    pub fn gap(&self) -> Option<i32> {
        match self {
            QualityGateOutcome::Unsatisfied { gap, .. } => Some(*gap),
            QualityGateOutcome::Satisfied { .. } => None,
        }
    }

    /// Returns a human-readable summary of this outcome.
    pub fn summary(&self) -> String {
        match self {
            QualityGateOutcome::Satisfied { required, observed } => {
                format!(
                    "Satisfied: observed {} meets required {}",
                    observed.as_str(),
                    required.as_str()
                )
            }
            QualityGateOutcome::Unsatisfied {
                required,
                observed,
                gap,
            } => {
                format!(
                    "Unsatisfied: observed {} < required {} (gap: {})",
                    observed.as_str(),
                    required.as_str(),
                    gap
                )
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_satisfied() {
        let outcome = QualityGateOutcome::Satisfied {
            required: QualityLevel::Package,
            observed: QualityLevel::Workspace,
        };
        assert!(outcome.is_satisfied());
        assert!(!outcome.is_unsatisfied());
    }

    #[test]
    fn test_is_unsatisfied() {
        let outcome = QualityGateOutcome::Unsatisfied {
            required: QualityLevel::Workspace,
            observed: QualityLevel::Package,
            gap: 1,
        };
        assert!(!outcome.is_satisfied());
        assert!(outcome.is_unsatisfied());
    }

    #[test]
    fn test_required_level() {
        let outcome = QualityGateOutcome::Satisfied {
            required: QualityLevel::Workspace,
            observed: QualityLevel::Workspace,
        };
        assert_eq!(outcome.required_level(), QualityLevel::Workspace);
    }

    #[test]
    fn test_observed_level() {
        let outcome = QualityGateOutcome::Unsatisfied {
            required: QualityLevel::MergeReady,
            observed: QualityLevel::TargetedTests,
            gap: 3,
        };
        assert_eq!(outcome.observed_level(), QualityLevel::TargetedTests);
    }

    #[test]
    fn test_gap_satisfied() {
        let outcome = QualityGateOutcome::Satisfied {
            required: QualityLevel::Package,
            observed: QualityLevel::Package,
        };
        assert_eq!(outcome.gap(), None);
    }

    #[test]
    fn test_gap_unsatisfied() {
        let outcome = QualityGateOutcome::Unsatisfied {
            required: QualityLevel::Workspace,
            observed: QualityLevel::TargetedTests,
            gap: 2,
        };
        assert_eq!(outcome.gap(), Some(2));
    }

    #[test]
    fn test_summary_satisfied() {
        let outcome = QualityGateOutcome::Satisfied {
            required: QualityLevel::Package,
            observed: QualityLevel::Workspace,
        };
        let s = outcome.summary();
        assert!(s.contains("Satisfied"));
        assert!(s.contains("workspace"));
        assert!(s.contains("package"));
    }

    #[test]
    fn test_summary_unsatisfied() {
        let outcome = QualityGateOutcome::Unsatisfied {
            required: QualityLevel::Workspace,
            observed: QualityLevel::TargetedTests,
            gap: 2,
        };
        let s = outcome.summary();
        assert!(s.contains("Unsatisfied"));
        assert!(s.contains("gap: 2"));
    }

    #[test]
    fn test_serialization_tagged() {
        let outcome = QualityGateOutcome::Satisfied {
            required: QualityLevel::Package,
            observed: QualityLevel::Package,
        };
        let json = serde_json::to_string(&outcome).unwrap();
        assert!(json.contains(r#""outcome":"satisfied""#));
    }

    #[test]
    fn test_serialization_roundtrip() {
        let variants = vec![
            QualityGateOutcome::Satisfied {
                required: QualityLevel::Package,
                observed: QualityLevel::Workspace,
            },
            QualityGateOutcome::Unsatisfied {
                required: QualityLevel::MergeReady,
                observed: QualityLevel::TargetedTests,
                gap: 3,
            },
        ];

        for variant in variants {
            let json = serde_json::to_string(&variant).unwrap();
            let deserialized: QualityGateOutcome = serde_json::from_str(&json).unwrap();
            assert_eq!(format!("{:?}", variant), format!("{:?}", deserialized));
        }
    }
}
