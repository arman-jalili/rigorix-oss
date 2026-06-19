//! QualityGateEvent — event payload schemas for the quality-gates bounded context.
//!
//! @canonical .pi/architecture/modules/quality-gates.md#event
//! Implements: Contract Freeze — QualityGateEvent payload schemas
//! Issue: #449 (quality-gates epic)
//!
//! These events are emitted on the `EventBus` whenever a quality gate is
//! evaluated. Consumers (audit, console printer, TUI) subscribe to these
//! event types.
//!
//! # Contract (Frozen)
//! - Each event carries the full context needed by consumers
//! - No internal implementation details exposed
//! - `sequence` is populated by EventBus at emission time

use serde::{Deserialize, Serialize};

use super::level::QualityLevel;
use super::outcome::QualityGateOutcome;

/// Events emitted by the Quality Gates module.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum QualityGateEvent {
    /// A quality gate was evaluated for a task.
    GateEvaluated {
        /// The required quality level from the contract.
        required: QualityLevel,
        /// The observed quality level from execution.
        observed: QualityLevel,
        /// The evaluation outcome.
        outcome: QualityGateOutcome,
        /// Optional task ID or node name for traceability.
        task_id: Option<String>,
    },

    /// A quality gate was satisfied — the task can proceed.
    GateSatisfied {
        /// The required quality level.
        required: QualityLevel,
        /// The observed quality level.
        observed: QualityLevel,
        /// Task ID for traceability.
        task_id: Option<String>,
    },

    /// A quality gate was not satisfied — broader testing is required.
    GateUnsatisfied {
        /// The required quality level.
        required: QualityLevel,
        /// The observed quality level.
        observed: QualityLevel,
        /// How many levels below requirement.
        gap: i32,
        /// Task ID for traceability.
        task_id: Option<String>,
    },

    /// The quality gate configuration was loaded or updated.
    ConfigUpdated {
        /// The default required level.
        default_level: QualityLevel,
        /// Number of template overrides.
        override_count: usize,
    },
}

impl QualityGateEvent {
    /// Returns a human-readable log line for this event.
    pub fn log_line(&self) -> String {
        match self {
            QualityGateEvent::GateEvaluated {
                required,
                observed,
                outcome,
                task_id,
            } => {
                let task = task_id
                    .as_deref()
                    .map(|id| format!(" [{}]", id))
                    .unwrap_or_default();
                format!(
                    "[QualityGate] Evaluated{}: required={}, observed={}, outcome={}",
                    task,
                    required.as_str(),
                    observed.as_str(),
                    if outcome.is_satisfied() {
                        "satisfied"
                    } else {
                        "unsatisfied"
                    }
                )
            }
            QualityGateEvent::GateSatisfied {
                required,
                observed,
                task_id,
            } => {
                let task = task_id
                    .as_deref()
                    .map(|id| format!(" [{}]", id))
                    .unwrap_or_default();
                format!(
                    "[QualityGate] Satisfied{}: {} >= {}",
                    task,
                    observed.as_str(),
                    required.as_str()
                )
            }
            QualityGateEvent::GateUnsatisfied {
                required,
                observed,
                gap,
                task_id,
            } => {
                let task = task_id
                    .as_deref()
                    .map(|id| format!(" [{}]", id))
                    .unwrap_or_default();
                format!(
                    "[QualityGate] Unsatisfied{}: {} < {} (gap: {})",
                    task,
                    observed.as_str(),
                    required.as_str(),
                    gap
                )
            }
            QualityGateEvent::ConfigUpdated {
                default_level,
                override_count,
            } => {
                format!(
                    "[QualityGate] Config updated: default={}, {} override(s)",
                    default_level.as_str(),
                    override_count
                )
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_line_evaluated() {
        let event = QualityGateEvent::GateEvaluated {
            required: QualityLevel::Workspace,
            observed: QualityLevel::Package,
            outcome: QualityGateOutcome::Unsatisfied {
                required: QualityLevel::Workspace,
                observed: QualityLevel::Package,
                gap: 1,
            },
            task_id: Some("task-1".to_string()),
        };
        let log = event.log_line();
        assert!(log.contains("Evaluated"));
        assert!(log.contains("workspace"));
        assert!(log.contains("package"));
        assert!(log.contains("task-1"));
    }

    #[test]
    fn test_log_line_satisfied() {
        let event = QualityGateEvent::GateSatisfied {
            required: QualityLevel::Package,
            observed: QualityLevel::Workspace,
            task_id: None,
        };
        let log = event.log_line();
        assert!(log.contains("Satisfied"));
    }

    #[test]
    fn test_log_line_unsatisfied() {
        let event = QualityGateEvent::GateUnsatisfied {
            required: QualityLevel::MergeReady,
            observed: QualityLevel::TargetedTests,
            gap: 3,
            task_id: Some("task-2".to_string()),
        };
        let log = event.log_line();
        assert!(log.contains("Unsatisfied"));
        assert!(log.contains("gap: 3"));
    }

    #[test]
    fn test_log_line_config_updated() {
        let event = QualityGateEvent::ConfigUpdated {
            default_level: QualityLevel::Workspace,
            override_count: 2,
        };
        let log = event.log_line();
        assert!(log.contains("Config updated"));
        assert!(log.contains("2 override(s)"));
    }

    #[test]
    fn test_serialization_roundtrip() {
        let events = vec![
            QualityGateEvent::GateEvaluated {
                required: QualityLevel::Workspace,
                observed: QualityLevel::Package,
                outcome: QualityGateOutcome::Unsatisfied {
                    required: QualityLevel::Workspace,
                    observed: QualityLevel::Package,
                    gap: 1,
                },
                task_id: None,
            },
            QualityGateEvent::GateSatisfied {
                required: QualityLevel::Package,
                observed: QualityLevel::Workspace,
                task_id: Some("t1".to_string()),
            },
            QualityGateEvent::GateUnsatisfied {
                required: QualityLevel::MergeReady,
                observed: QualityLevel::TargetedTests,
                gap: 3,
                task_id: None,
            },
            QualityGateEvent::ConfigUpdated {
                default_level: QualityLevel::Package,
                override_count: 0,
            },
        ];

        for event in events {
            let json = serde_json::to_string(&event).unwrap();
            let deserialized: QualityGateEvent = serde_json::from_str(&json).unwrap();
            assert_eq!(format!("{:?}", event), format!("{:?}", deserialized));
        }
    }
}
