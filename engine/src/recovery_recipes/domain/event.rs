//! RecoveryEvent — event payload schemas for the recovery-recipes bounded context.
//!
//! @canonical .pi/architecture/modules/recovery-recipes.md#event
//! Implements: Contract Freeze — RecoveryEvent payload schemas
//! Issue: #438 (recovery-recipes epic)
//!
//! These events are emitted on the `EventBus` whenever a recovery attempt
//! is made, succeeds, fails, or requires escalation. Consumers (audit,
//! console printer, TUI) subscribe to these event types.
//!
//! # Contract (Frozen)
//! - Each event carries the full context needed by consumers
//! - No internal implementation details exposed
//! - `sequence` is populated by EventBus at emission time

use serde::{Deserialize, Serialize};

use super::result::RecoveryResult;
use super::scenario::FailureScenario;
use super::step::RecoveryStep;

/// Events emitted by the Recovery Recipes module.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RecoveryEvent {
    /// A recovery attempt was initiated for a failure scenario.
    RecoveryAttempted {
        /// The failure scenario being recovered.
        scenario: FailureScenario,
        /// The recipe step being attempted.
        step: RecoveryStep,
        /// Attempt number (1-based).
        attempt_number: u32,
    },

    /// A recovery attempt completed successfully.
    RecoverySucceeded {
        /// The failure scenario that was recovered.
        scenario: FailureScenario,
        /// Number of steps taken to recover.
        steps_taken: u32,
        /// The final recovery result.
        result: RecoveryResult,
    },

    /// A recovery attempt failed.
    RecoveryFailed {
        /// The failure scenario that could not be recovered.
        scenario: FailureScenario,
        /// The step that failed.
        failed_step: RecoveryStep,
        /// Human-readable reason for the failure.
        reason: String,
        /// Whether this was the final attempt before escalation.
        is_final_attempt: bool,
    },

    /// A recovery was escalated (all automatic attempts exhausted).
    Escalated {
        /// The failure scenario that was escalated.
        scenario: FailureScenario,
        /// Total number of attempts made before escalation.
        attempts_made: u32,
        /// Human-readable reason for escalation.
        reason: String,
    },

    /// A recovery recipe was not found for a scenario.
    /// This is an informational event — the execution engine will
    /// escalate the failure to the node's standard retry logic.
    RecipeNotFound {
        /// The failure scenario that has no recipe.
        scenario: FailureScenario,
        /// The error message from the original failure.
        original_error: String,
    },
}

impl RecoveryEvent {
    /// Returns a human-readable log line for this event.
    pub fn log_line(&self) -> String {
        match self {
            RecoveryEvent::RecoveryAttempted {
                scenario,
                step,
                attempt_number,
            } => {
                format!(
                    "[Recovery] Attempt #{} for {:?}: {:?}",
                    attempt_number, scenario, step
                )
            }
            RecoveryEvent::RecoverySucceeded {
                scenario,
                steps_taken,
                ..
            } => {
                format!(
                    "[Recovery] Succeeded for {:?} ({} step(s))",
                    scenario, steps_taken
                )
            }
            RecoveryEvent::RecoveryFailed {
                scenario,
                failed_step,
                reason,
                is_final_attempt,
            } => {
                let final_tag = if *is_final_attempt { " [FINAL]" } else { "" };
                format!(
                    "[Recovery] Failed for {:?} at step {:?}: {}{}",
                    scenario, failed_step, reason, final_tag
                )
            }
            RecoveryEvent::Escalated {
                scenario,
                attempts_made,
                reason,
            } => {
                format!(
                    "[Recovery] Escalated for {:?} after {} attempt(s): {}",
                    scenario, attempts_made, reason
                )
            }
            RecoveryEvent::RecipeNotFound {
                scenario,
                original_error,
            } => {
                format!(
                    "[Recovery] No recipe for {:?}: {}",
                    scenario, original_error
                )
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_line_attempted() {
        let event = RecoveryEvent::RecoveryAttempted {
            scenario: FailureScenario::CompileError,
            step: RecoveryStep::CleanBuild,
            attempt_number: 1,
        };
        let log = event.log_line();
        assert!(log.contains("CompileError"));
        assert!(log.contains("CleanBuild"));
        assert!(log.contains("Attempt #1"));
    }

    #[test]
    fn test_log_line_succeeded() {
        let event = RecoveryEvent::RecoverySucceeded {
            scenario: FailureScenario::CompileError,
            steps_taken: 1,
            result: RecoveryResult::Recovered { steps_taken: 1 },
        };
        let log = event.log_line();
        assert!(log.contains("Succeeded"));
        assert!(log.contains("CompileError"));
    }

    #[test]
    fn test_log_line_failed() {
        let event = RecoveryEvent::RecoveryFailed {
            scenario: FailureScenario::TestFailure,
            failed_step: RecoveryStep::ExpandContext,
            reason: "context window full".to_string(),
            is_final_attempt: true,
        };
        let log = event.log_line();
        assert!(log.contains("Failed"));
        assert!(log.contains("TestFailure"));
        assert!(log.contains("FINAL"));
    }

    #[test]
    fn test_log_line_escalated() {
        let event = RecoveryEvent::Escalated {
            scenario: FailureScenario::ProviderFailure,
            attempts_made: 2,
            reason: "provider still unavailable".to_string(),
        };
        let log = event.log_line();
        assert!(log.contains("Escalated"));
        assert!(log.contains("ProviderFailure"));
    }

    #[test]
    fn test_log_line_recipe_not_found() {
        let event = RecoveryEvent::RecipeNotFound {
            scenario: FailureScenario::CompileError,
            original_error: "build failed".to_string(),
        };
        let log = event.log_line();
        assert!(log.contains("No recipe"));
    }

    #[test]
    fn test_serialization_roundtrip() {
        let events = vec![
            RecoveryEvent::RecoveryAttempted {
                scenario: FailureScenario::CompileError,
                step: RecoveryStep::CleanBuild,
                attempt_number: 1,
            },
            RecoveryEvent::RecoverySucceeded {
                scenario: FailureScenario::CompileError,
                steps_taken: 1,
                result: RecoveryResult::Recovered { steps_taken: 1 },
            },
            RecoveryEvent::RecoveryFailed {
                scenario: FailureScenario::TestFailure,
                failed_step: RecoveryStep::ExpandContext,
                reason: "test failure".to_string(),
                is_final_attempt: true,
            },
            RecoveryEvent::Escalated {
                scenario: FailureScenario::ProviderFailure,
                attempts_made: 2,
                reason: "exhausted".to_string(),
            },
            RecoveryEvent::RecipeNotFound {
                scenario: FailureScenario::CompileError,
                original_error: "build error".to_string(),
            },
        ];

        for event in events {
            let json = serde_json::to_string(&event).unwrap();
            let deserialized: RecoveryEvent = serde_json::from_str(&json).unwrap();
            assert_eq!(format!("{:?}", event), format!("{:?}", deserialized));
        }
    }
}
