//! PolicyCondition domain entity.
//!
//! @canonical .pi/architecture/modules/policy-engine.md#condition
//! Implements: Contract Freeze — PolicyCondition enum
//! Issue: issue-contract-freeze
//!
//! Composable boolean conditions evaluated against LaneContext.
//! Supports logical combinators (And/Or) and leaf conditions over
//! observable state — quality level, branch freshness, review status,
//! completion state, and diff scope.
//!
//! # Contract (Frozen)
//! - All conditions are pure boolean predicates over LaneContext
//! - And/Or enable arbitrarily nested condition trees
//! - `GreenAt` bridges policy engine to QualityGates
//! - No framework-specific annotations — pure domain types

use serde::{Deserialize, Serialize};

/// Composable boolean conditions evaluated against LaneContext.
///
/// These are the building blocks of policy rules. Conditions can be
/// simple leaf checks (`LaneCompleted`, `StaleBranch`) or compound
/// boolean trees using `And` and `Or`.
///
/// # Serde Tagging
///
/// Uses `#[serde(tag = "type", rename_all = "snake_case")]` for
/// TOML/JSON compatibility. Examples:
///
/// ```toml
/// # Compound And condition
/// condition = { type = "and", conditions = [
///     { type = "lane_completed" },
///     { type = "green_at", level = 3 }
/// ]}
///
/// # Simple condition
/// condition = { type = "stale_branch" }
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PolicyCondition {
    /// All sub-conditions must match (logical AND).
    ///
    /// Returns `true` only if every sub-condition evaluates to `true`.
    /// Short-circuits on the first `false`.
    And {
        /// The list of sub-conditions that must all match.
        conditions: Vec<PolicyCondition>,
    },

    /// Any sub-condition must match (logical OR).
    ///
    /// Returns `true` if at least one sub-condition evaluates to `true`.
    /// Short-circuits on the first `true`.
    Or {
        /// The list of sub-conditions where at least one must match.
        conditions: Vec<PolicyCondition>,
    },

    /// Quality gate is at or above the given level.
    ///
    /// The level is a u8 representing the quality gate tier (1-5).
    /// Level 0 matches any green gate (equivalent to "at least green").
    GreenAt {
        /// The minimum quality level required.
        level: u8,
    },

    /// Branch has been stale beyond the configured threshold.
    ///
    /// Evaluates `branch_freshness_secs` against a configurable threshold
    /// (defined by the threshold in LaneContext or a rule-level config).
    /// Returns `true` if the branch has received no commits for longer
    /// than the threshold.
    StaleBranch,

    /// Lane is blocked at startup (e.g., dependency not met).
    ///
    /// Matches when `LaneContext.blocker` is `LaneBlocker::Startup`.
    StartupBlocked,

    /// Lane execution has completed (all nodes executed).
    ///
    /// Matches when `LaneContext.completed` is `true`.
    LaneCompleted,

    /// Lane has been reconciled (no merge needed).
    ///
    /// Matches when `LaneContext.reconciled` is `true`.
    LaneReconciled,

    /// Review has been approved for this lane's diff.
    ///
    /// Matches when `LaneContext.review_status` is `ReviewStatus::Approved`.
    ReviewPassed,

    /// Diff is scoped (not a full-repo diff).
    ///
    /// Matches when `LaneContext.diff_scope` is `DiffScope::Scoped`.
    ScopedDiff,

    /// Branch has been untouched for the specified duration.
    ///
    /// Evaluates `branch_freshness_secs >= duration_secs`.
    TimedOut {
        /// Minimum duration in seconds since the last commit.
        duration_secs: u64,
    },

    /// All scoring dimensions are above a percentage threshold.
    ///
    /// Evaluates against `LaneContext::scoring_scores`. If `dimension` is
    /// `None`, checks all dimensions. If `dimension` is `Some`, checks only
    /// that specific dimension.
    ///
    /// Score thresholds use u8 percentage (0–100), matching the `GreenAt`
    /// convention. Backend scores (0.0–1.0) are converted: `(score * 100.0) as u8`.
    ScoreAbove {
        /// Optional: only check this specific dimension. None = all dimensions.
        dimension: Option<String>,
        /// Minimum score threshold as percentage (0–100). E.g., 80 = 80%.
        threshold: u8,
    },

    /// Any scoring dimension below a percentage threshold triggers action.
    ///
    /// Evaluates against `LaneContext::scoring_scores`. If `dimension` is
    /// `None`, checks any dimension. If `dimension` is `Some`, checks only
    /// that specific dimension.
    ScoreBelow {
        /// Optional: only check this specific dimension. None = any dimension.
        dimension: Option<String>,
        /// Minimum score threshold as percentage (0–100). E.g., 80 = 80%.
        threshold: u8,
    },
}

impl PolicyCondition {
    /// Evaluate this condition against a LaneContext.
    ///
    /// This is a pure domain method with no side effects.
    /// Returns `true` if the condition matches the context.
    pub fn matches(&self, context: &super::context::LaneContext) -> bool {
        match self {
            PolicyCondition::And { conditions } => conditions.iter().all(|c| c.matches(context)),
            PolicyCondition::Or { conditions } => conditions.iter().any(|c| c.matches(context)),
            PolicyCondition::GreenAt { level } => context.green_level >= *level,
            PolicyCondition::StaleBranch => {
                // Staleness threshold is derived from the context's
                // branch_freshness_secs. A branch is stale if its freshness
                // exceeds a configurable threshold (default: 7 days = 604800 secs).
                context.branch_freshness_secs > 604_800
            }
            PolicyCondition::StartupBlocked => {
                context.blocker == super::context::LaneBlocker::Startup
            }
            PolicyCondition::LaneCompleted => context.completed,
            PolicyCondition::LaneReconciled => context.reconciled,
            PolicyCondition::ReviewPassed => {
                context.review_status == super::context::ReviewStatus::Approved
            }
            PolicyCondition::ScopedDiff => context.diff_scope == super::context::DiffScope::Scoped,
            PolicyCondition::TimedOut { duration_secs } => {
                context.branch_freshness_secs >= *duration_secs
            }
            PolicyCondition::ScoreAbove {
                dimension,
                threshold,
            } => {
                let threshold = *threshold;
                match dimension {
                    Some(dim) => context.scoring_scores.get(dim).copied().map_or(false, |s| s >= threshold),
                    None => {
                        if context.scoring_scores.is_empty() {
                            false
                        } else {
                            context.scoring_scores.values().all(|&s| s >= threshold)
                        }
                    }
                }
            }
            PolicyCondition::ScoreBelow {
                dimension,
                threshold,
            } => {
                let threshold = *threshold;
                match dimension {
                    Some(dim) => context.scoring_scores.get(dim).copied().map_or(true, |s| s < threshold),
                    None => context.scoring_scores.values().any(|&s| s < threshold),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::policy_engine::domain::{
        PolicyCondition,
        context::{DiffScope, LaneBlocker, LaneContext, ReviewStatus},
    };

    fn test_context() -> LaneContext {
        let mut scores = std::collections::HashMap::new();
        scores.insert("correctness".to_string(), 85);
        scores.insert("completeness".to_string(), 90);
        LaneContext {
            lane_id: "test-lane".to_string(),
            green_level: 3,
            branch_freshness_secs: 100,
            blocker: LaneBlocker::None,
            review_status: ReviewStatus::Pending,
            diff_scope: DiffScope::Scoped,
            completed: true,
            reconciled: false,
            scoring_scores: scores,
        }
    }

    #[test]
    fn test_lane_completed_matches() {
        let ctx = test_context();
        assert!(PolicyCondition::LaneCompleted.matches(&ctx));
    }

    #[test]
    fn test_lane_completed_does_not_match() {
        let mut ctx = test_context();
        ctx.completed = false;
        assert!(!PolicyCondition::LaneCompleted.matches(&ctx));
    }

    #[test]
    fn test_green_at_matches() {
        let ctx = test_context();
        assert!(PolicyCondition::GreenAt { level: 3 }.matches(&ctx));
        assert!(PolicyCondition::GreenAt { level: 2 }.matches(&ctx));
    }

    #[test]
    fn test_green_at_does_not_match() {
        let ctx = test_context();
        assert!(!PolicyCondition::GreenAt { level: 4 }.matches(&ctx));
    }

    #[test]
    fn test_and_all_true() {
        let ctx = test_context();
        let condition = PolicyCondition::And {
            conditions: vec![PolicyCondition::LaneCompleted, PolicyCondition::ScopedDiff],
        };
        assert!(condition.matches(&ctx));
    }

    #[test]
    fn test_and_one_false() {
        let ctx = test_context();
        let condition = PolicyCondition::And {
            conditions: vec![PolicyCondition::LaneCompleted, PolicyCondition::StaleBranch],
        };
        assert!(!condition.matches(&ctx));
    }

    #[test]
    fn test_or_one_true() {
        let ctx = test_context();
        let condition = PolicyCondition::Or {
            conditions: vec![PolicyCondition::StaleBranch, PolicyCondition::LaneCompleted],
        };
        assert!(condition.matches(&ctx));
    }

    #[test]
    fn test_or_all_false() {
        let mut ctx = test_context();
        ctx.completed = false;
        let condition = PolicyCondition::Or {
            conditions: vec![PolicyCondition::StaleBranch, PolicyCondition::LaneCompleted],
        };
        assert!(!condition.matches(&ctx));
    }

    #[test]
    fn test_startup_blocked_matches() {
        let mut ctx = test_context();
        ctx.blocker = LaneBlocker::Startup;
        assert!(PolicyCondition::StartupBlocked.matches(&ctx));
    }

    #[test]
    fn test_review_passed_matches() {
        let mut ctx = test_context();
        ctx.review_status = ReviewStatus::Approved;
        assert!(PolicyCondition::ReviewPassed.matches(&ctx));
    }

    #[test]
    fn test_timed_out_matches() {
        let mut ctx = test_context();
        ctx.branch_freshness_secs = 3600;
        assert!(
            PolicyCondition::TimedOut {
                duration_secs: 1800
            }
            .matches(&ctx)
        );
    }

    #[test]
    fn test_timed_out_does_not_match() {
        let ctx = test_context();
        assert!(!PolicyCondition::TimedOut { duration_secs: 200 }.matches(&ctx));
    }

    #[test]
    fn test_condition_serde_roundtrip() {
        let condition = PolicyCondition::And {
            conditions: vec![
                PolicyCondition::LaneCompleted,
                PolicyCondition::GreenAt { level: 3 },
            ],
        };
        let json = serde_json::to_string(&condition).unwrap();
        let deserialized: PolicyCondition = serde_json::from_str(&json).unwrap();
        assert_eq!(condition, deserialized);
    }

    #[test]
    fn test_stale_branch_matches() {
        let mut ctx = test_context();
        ctx.branch_freshness_secs = 604_801; // > 7 days
        assert!(PolicyCondition::StaleBranch.matches(&ctx));
    }

    #[test]
    fn test_stale_branch_does_not_match() {
        let ctx = test_context();
        assert!(!PolicyCondition::StaleBranch.matches(&ctx));
    }

    #[test]
    fn test_lane_reconciled_matches() {
        let mut ctx = test_context();
        ctx.reconciled = true;
        assert!(PolicyCondition::LaneReconciled.matches(&ctx));
    }

    #[test]
    fn test_scoped_diff_matches() {
        let ctx = test_context();
        assert!(PolicyCondition::ScopedDiff.matches(&ctx));
    }

    #[test]
    fn test_scoped_diff_does_not_match() {
        let mut ctx = test_context();
        ctx.diff_scope = DiffScope::Full;
        assert!(!PolicyCondition::ScopedDiff.matches(&ctx));
    }

    #[test]
    fn test_score_above_all_dimensions() {
        let ctx = test_context();
        assert!(PolicyCondition::ScoreAbove {
            dimension: None,
            threshold: 80,
        }
        .matches(&ctx));
    }

    #[test]
    fn test_score_above_one_fails() {
        let ctx = test_context();
        assert!(!PolicyCondition::ScoreAbove {
            dimension: None,
            threshold: 95,
        }
        .matches(&ctx));
    }

    #[test]
    fn test_score_above_specific_dimension() {
        let ctx = test_context();
        assert!(PolicyCondition::ScoreAbove {
            dimension: Some("correctness".to_string()),
            threshold: 80,
        }
        .matches(&ctx));
        assert!(!PolicyCondition::ScoreAbove {
            dimension: Some("correctness".to_string()),
            threshold: 90,
        }
        .matches(&ctx));
    }

    #[test]
    fn test_score_above_missing_dimension() {
        let ctx = test_context();
        assert!(!PolicyCondition::ScoreAbove {
            dimension: Some("nonexistent".to_string()),
            threshold: 50,
        }
        .matches(&ctx));
    }

    #[test]
    fn test_score_below_any_dimension() {
        let mut ctx = test_context();
        ctx.scoring_scores.insert("style".to_string(), 70);
        assert!(PolicyCondition::ScoreBelow {
            dimension: None,
            threshold: 80,
        }
        .matches(&ctx));
    }

    #[test]
    fn test_score_below_all_pass() {
        let ctx = test_context();
        assert!(!PolicyCondition::ScoreBelow {
            dimension: None,
            threshold: 80,
        }
        .matches(&ctx));
    }

    #[test]
    fn test_score_below_specific_dimension() {
        let mut ctx = test_context();
        ctx.scoring_scores.insert("style".to_string(), 70);
        assert!(PolicyCondition::ScoreBelow {
            dimension: Some("style".to_string()),
            threshold: 80,
        }
        .matches(&ctx));
        assert!(!PolicyCondition::ScoreBelow {
            dimension: Some("correctness".to_string()),
            threshold: 80,
        }
        .matches(&ctx));
    }

    #[test]
    fn test_score_above_empty_scores() {
        let mut ctx = test_context();
        ctx.scoring_scores.clear();
        assert!(!PolicyCondition::ScoreAbove {
            dimension: None,
            threshold: 50,
        }
        .matches(&ctx));
    }

    #[test]
    fn test_score_above_serde_roundtrip() {
        let condition = PolicyCondition::ScoreAbove {
            dimension: Some("correctness".to_string()),
            threshold: 80,
        };
        let json = serde_json::to_string(&condition).unwrap();
        let deserialized: PolicyCondition = serde_json::from_str(&json).unwrap();
        assert_eq!(condition, deserialized);
    }

    #[test]
    fn test_score_below_serde_roundtrip() {
        let condition = PolicyCondition::ScoreBelow {
            dimension: None,
            threshold: 80,
        };
        let json = serde_json::to_string(&condition).unwrap();
        let deserialized: PolicyCondition = serde_json::from_str(&json).unwrap();
        assert_eq!(condition, deserialized);
    }
}
