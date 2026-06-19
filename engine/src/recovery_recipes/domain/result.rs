//! RecoveryResult — outcome of a recovery recipe execution.
//!
//! @canonical .pi/architecture/modules/recovery-recipes.md#recoveryresult
//! Implements: Contract Freeze — RecoveryResult enum
//! Issue: #438 (recovery-recipes epic)
//!
//! # Contract (Frozen)
//! - Three mutually exclusive outcomes
//! - `Recovered` — all steps completed, node can re-execute
//! - `PartialRecovery` — some steps succeeded, node might still recover
//! - `EscalationRequired` — automatic recovery exhausted, escalation needed
//! - Implements `Clone`, `Debug`, `PartialEq`, `Eq` for testability
//! - Serialization support for eventing and API responses

use serde::{Deserialize, Serialize};

use super::step::RecoveryStep;

/// Outcome of executing a `RecoveryRecipe` for a given `FailureScenario`.
///
/// The result determines how the execution engine proceeds:
/// - `Recovered`: re-execute the failed node
/// - `PartialRecovery`: re-execute with partial state (some steps succeeded)
/// - `EscalationRequired`: apply escalation policy (alert, skip, or abort)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryResult {
    /// All recovery steps completed successfully.
    /// The execution engine should re-execute the failed node.
    Recovered {
        /// Number of recovery steps that were executed.
        steps_taken: u32,
    },

    /// Some steps succeeded, some could not run or failed.
    /// The execution engine may re-execute with partial recovery state.
    PartialRecovery {
        /// Steps that completed successfully.
        recovered: Vec<RecoveryStep>,
        /// Steps that could not be executed or failed.
        remaining: Vec<RecoveryStep>,
    },

    /// Automatic recovery attempts exhausted — escalation is required.
    /// The execution engine should apply the recipe's `EscalationPolicy`.
    EscalationRequired {
        /// Human-readable reason for the escalation.
        reason: String,
    },
}

impl RecoveryResult {
    /// Returns `true` if this result indicates successful recovery.
    pub fn is_recovered(&self) -> bool {
        matches!(self, RecoveryResult::Recovered { .. })
    }

    /// Returns `true` if this result indicates partial recovery.
    pub fn is_partial(&self) -> bool {
        matches!(self, RecoveryResult::PartialRecovery { .. })
    }

    /// Returns `true` if escalation is required.
    pub fn is_escalation_required(&self) -> bool {
        matches!(self, RecoveryResult::EscalationRequired { .. })
    }

    /// Returns the number of steps that were successfully executed.
    pub fn steps_executed(&self) -> u32 {
        match self {
            RecoveryResult::Recovered { steps_taken } => *steps_taken,
            RecoveryResult::PartialRecovery { recovered, .. } => recovered.len() as u32,
            RecoveryResult::EscalationRequired { .. } => 0,
        }
    }

    /// Returns a human-readable summary of this result.
    pub fn summary(&self) -> String {
        match self {
            RecoveryResult::Recovered { steps_taken } => {
                format!("Recovered after {} step(s)", steps_taken)
            }
            RecoveryResult::PartialRecovery { recovered, remaining } => {
                format!(
                    "Partial recovery: {} succeeded, {} remaining",
                    recovered.len(),
                    remaining.len()
                )
            }
            RecoveryResult::EscalationRequired { reason } => {
                format!("Escalation required: {}", reason)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_recovered() {
        let result = RecoveryResult::Recovered { steps_taken: 2 };
        assert!(result.is_recovered());
        assert!(!result.is_partial());
        assert!(!result.is_escalation_required());
    }

    #[test]
    fn test_is_partial() {
        let result = RecoveryResult::PartialRecovery {
            recovered: vec![RecoveryStep::CleanBuild],
            remaining: vec![RecoveryStep::ExpandContext],
        };
        assert!(!result.is_recovered());
        assert!(result.is_partial());
        assert!(!result.is_escalation_required());
    }

    #[test]
    fn test_is_escalation_required() {
        let result = RecoveryResult::EscalationRequired {
            reason: "max attempts reached".to_string(),
        };
        assert!(!result.is_recovered());
        assert!(!result.is_partial());
        assert!(result.is_escalation_required());
    }

    #[test]
    fn test_steps_executed() {
        assert_eq!(
            RecoveryResult::Recovered { steps_taken: 3 }.steps_executed(),
            3
        );
        assert_eq!(
            RecoveryResult::PartialRecovery {
                recovered: vec![RecoveryStep::CleanBuild],
                remaining: vec![RecoveryStep::ExpandContext],
            }
            .steps_executed(),
            1
        );
        assert_eq!(
            RecoveryResult::EscalationRequired {
                reason: "test".to_string()
            }
            .steps_executed(),
            0
        );
    }

    #[test]
    fn test_summary() {
        let result = RecoveryResult::Recovered { steps_taken: 1 };
        assert_eq!(result.summary(), "Recovered after 1 step(s)");

        let result = RecoveryResult::EscalationRequired {
            reason: "max".to_string(),
        };
        assert!(result.summary().contains("Escalation required"));
    }

    #[test]
    fn test_serialization_roundtrip() {
        let variants = vec![
            RecoveryResult::Recovered { steps_taken: 2 },
            RecoveryResult::PartialRecovery {
                recovered: vec![RecoveryStep::CleanBuild],
                remaining: vec![RecoveryStep::ExpandContext],
            },
            RecoveryResult::EscalationRequired {
                reason: "max attempts".to_string(),
            },
        ];

        for variant in variants {
            let json = serde_json::to_string(&variant).unwrap();
            let deserialized: RecoveryResult = serde_json::from_str(&json).unwrap();
            assert_eq!(format!("{:?}", variant), format!("{:?}", deserialized));
        }
    }
}
