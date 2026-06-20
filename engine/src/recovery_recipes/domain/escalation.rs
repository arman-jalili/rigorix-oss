//! EscalationPolicy — what to do when recovery attempts are exhausted.
//!
//! @canonical .pi/architecture/modules/recovery-recipes.md#escalation
//! Implements: Contract Freeze — EscalationPolicy enum
//! Issue: #438 (recovery-recipes epic)
//!
//! # Contract (Frozen)
//! - Three mutually exclusive escalation behaviors
//! - `AlertHuman` notifies operators and continues execution
//! - `LogAndContinue` silently logs and moves on
//! - `Abort` terminates the executing session
//! - Implements `Clone`, `Debug`, `PartialEq`, `Eq` for testability
//! - Serialization support for configuration and API responses

use serde::{Deserialize, Serialize};

/// Defines what action to take when automatic recovery attempts are
/// exhausted for a given `FailureScenario`.
///
/// Attached to each `RecoveryRecipe` as its `escalation_policy`.
/// The policy is evaluated after `max_attempts` have been consumed
/// without a successful recovery.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EscalationPolicy {
    /// Alert a human operator (e.g., via notification, event, or log)
    /// and allow execution to continue (the failed node is marked as Failed).
    AlertHuman,

    /// Log the exhaustion event and continue execution silently.
    /// Use sparingly — can mask systemic issues.
    LogAndContinue,

    /// Abort the entire execution session immediately.
    /// Reserved for scenarios where continuation would cause data loss
    /// or cascading failures.
    Abort,
}

impl EscalationPolicy {
    /// Returns the canonical snake_case name of this policy.
    pub fn as_str(&self) -> &'static str {
        match self {
            EscalationPolicy::AlertHuman => "alert_human",
            EscalationPolicy::LogAndContinue => "log_and_continue",
            EscalationPolicy::Abort => "abort",
        }
    }

    /// Returns a human-readable description of this policy.
    pub fn description(&self) -> &'static str {
        match self {
            EscalationPolicy::AlertHuman => "Alert a human operator and continue",
            EscalationPolicy::LogAndContinue => "Log and continue execution silently",
            EscalationPolicy::Abort => "Abort the entire execution session",
        }
    }

    /// Returns `true` if execution can continue after this policy is applied.
    pub fn allows_continuation(&self) -> bool {
        matches!(
            self,
            EscalationPolicy::AlertHuman | EscalationPolicy::LogAndContinue
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_as_str() {
        assert_eq!(EscalationPolicy::AlertHuman.as_str(), "alert_human");
        assert_eq!(
            EscalationPolicy::LogAndContinue.as_str(),
            "log_and_continue"
        );
        assert_eq!(EscalationPolicy::Abort.as_str(), "abort");
    }

    #[test]
    fn test_description() {
        assert!(!EscalationPolicy::AlertHuman.description().is_empty());
        assert!(!EscalationPolicy::LogAndContinue.description().is_empty());
        assert!(!EscalationPolicy::Abort.description().is_empty());
    }

    #[test]
    fn test_allows_continuation() {
        assert!(EscalationPolicy::AlertHuman.allows_continuation());
        assert!(EscalationPolicy::LogAndContinue.allows_continuation());
        assert!(!EscalationPolicy::Abort.allows_continuation());
    }

    #[test]
    fn test_serialization_roundtrip() {
        for variant in &[
            EscalationPolicy::AlertHuman,
            EscalationPolicy::LogAndContinue,
            EscalationPolicy::Abort,
        ] {
            let json = serde_json::to_string(variant).unwrap();
            let deserialized: EscalationPolicy = serde_json::from_str(&json).unwrap();
            assert_eq!(*variant, deserialized);
        }
    }
}
