//! Event payload schemas for the Policy Engine bounded context.
//!
//! @canonical .pi/architecture/modules/policy-engine.md#event
//! Implements: Contract Freeze — PolicyEvent payload schemas
//! Issue: issue-contract-freeze
//!
//! These events are emitted whenever policy evaluation occurs —
//! rules matched, actions dispatched, configuration loaded.
//! Consumers (orchestrator, audit, TUI) subscribe to these event types.
//!
//! # Contract (Frozen)
//! - Each event carries the full context needed by consumers
//! - No internal implementation details exposed
//! - `lane_id` correlates to the originating lane

use serde::{Deserialize, Serialize};

use super::action::PolicyAction;

/// Events emitted by the Policy Engine module.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PolicyEvent {
    /// A policy rule matched the current LaneContext.
    ///
    /// Emitted for every matching rule during evaluation.
    RuleMatched {
        /// The lane ID for which the rule matched.
        lane_id: String,
        /// The name of the matching rule.
        rule_name: String,
        /// The priority of the matching rule.
        priority: u32,
        /// The action produced by this matching rule.
        action: PolicyAction,
    },

    /// A flat list of actions was produced by evaluating a LaneContext.
    ///
    /// Emitted once per evaluate() call, containing all actions from
    /// all matching rules in priority order.
    ActionsDispatched {
        /// The lane ID for which actions were produced.
        lane_id: String,
        /// The flat list of actions in dispatch order.
        actions: Vec<PolicyAction>,
        /// Number of rules that matched this evaluation.
        matching_rule_count: u32,
    },

    /// The policy configuration was loaded or reloaded.
    ConfigLoaded {
        /// The source from which the config was loaded (e.g., "file", "repository").
        source: String,
        /// Number of rules loaded.
        rule_count: u32,
        /// Whether the load was successful.
        success: bool,
        /// Error details if the load failed.
        error: Option<String>,
    },

    /// A policy evaluation was performed for a LaneContext.
    ///
    /// Emitted for every evaluation, whether or not any rules matched.
    EvaluationPerformed {
        /// The lane ID that was evaluated.
        lane_id: String,
        /// Whether any rules matched.
        matched: bool,
        /// Number of rules evaluated (total).
        rules_evaluated: u32,
        /// Duration of the evaluation in milliseconds.
        duration_ms: u64,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::policy_engine::domain::PolicyAction;

    #[test]
    fn test_rule_matched_event_serde() {
        let event = PolicyEvent::RuleMatched {
            lane_id: "lane-1".to_string(),
            rule_name: "closeout-completed".to_string(),
            priority: 10,
            action: PolicyAction::CloseoutLane,
        };
        let json = serde_json::to_string(&event).unwrap();
        let deserialized: PolicyEvent = serde_json::from_str(&json).unwrap();
        assert!(matches!(deserialized, PolicyEvent::RuleMatched { .. }));
    }

    #[test]
    fn test_actions_dispatched_event() {
        let event = PolicyEvent::ActionsDispatched {
            lane_id: "lane-1".to_string(),
            actions: vec![PolicyAction::CloseoutLane, PolicyAction::CleanupSession],
            matching_rule_count: 2,
        };
        let json = serde_json::to_string(&event).unwrap();
        let deserialized: PolicyEvent = serde_json::from_str(&json).unwrap();
        assert!(matches!(
            deserialized,
            PolicyEvent::ActionsDispatched { .. }
        ));
    }

    #[test]
    fn test_config_loaded_event() {
        let event = PolicyEvent::ConfigLoaded {
            source: ".rigorix/policy.toml".to_string(),
            rule_count: 5,
            success: true,
            error: None,
        };
        let json = serde_json::to_string(&event).unwrap();
        let deserialized: PolicyEvent = serde_json::from_str(&json).unwrap();
        assert!(matches!(deserialized, PolicyEvent::ConfigLoaded { .. }));
    }

    #[test]
    fn test_evaluation_performed_event() {
        let event = PolicyEvent::EvaluationPerformed {
            lane_id: "lane-1".to_string(),
            matched: true,
            rules_evaluated: 10,
            duration_ms: 5,
        };
        let json = serde_json::to_string(&event).unwrap();
        let deserialized: PolicyEvent = serde_json::from_str(&json).unwrap();
        assert!(matches!(
            deserialized,
            PolicyEvent::EvaluationPerformed { .. }
        ));
    }

    #[test]
    fn test_serde_roundtrip_all_variants() {
        let variants = vec![
            PolicyEvent::RuleMatched {
                lane_id: "l".to_string(),
                rule_name: "r".to_string(),
                priority: 1,
                action: PolicyAction::CloseoutLane,
            },
            PolicyEvent::ActionsDispatched {
                lane_id: "l".to_string(),
                actions: vec![],
                matching_rule_count: 0,
            },
            PolicyEvent::ConfigLoaded {
                source: "s".to_string(),
                rule_count: 0,
                success: true,
                error: None,
            },
            PolicyEvent::EvaluationPerformed {
                lane_id: "l".to_string(),
                matched: false,
                rules_evaluated: 0,
                duration_ms: 0,
            },
        ];

        for event in variants {
            let json = serde_json::to_string(&event).unwrap();
            let deserialized: PolicyEvent = serde_json::from_str(&json).unwrap();
            assert!(std::mem::discriminant(&event) == std::mem::discriminant(&deserialized));
        }
    }
}
