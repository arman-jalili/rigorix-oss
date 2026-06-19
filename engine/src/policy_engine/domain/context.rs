//! LaneContext domain entity.
//!
//! @canonical .pi/architecture/modules/policy-engine.md#context
//! Implements: Contract Freeze — LaneContext struct
//! Issue: issue-contract-freeze
//!
//! A typed snapshot of execution state that policy conditions evaluate
//! against. Built by the orchestrator from ExecutionRecord, git state,
//! risk gating, and review state.
//!
//! # Contract (Frozen)
//! - All fields are public for direct condition evaluation
//! - Context is immutable once constructed
//! - `green_level` is a u8 representing the quality gate tier (0-5)

use serde::{Deserialize, Serialize};

/// Typed snapshot of execution state evaluated by policy conditions.
///
/// The LaneContext is constructed by the orchestrator after execution
/// completes, before the closeout phase. It aggregates all observable
/// state needed by policy conditions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LaneContext {
    /// Unique identifier for this lane (execution session).
    pub lane_id: String,

    /// The quality gate level achieved (0-5, where 0 = no gate, 5 = highest).
    pub green_level: u8,

    /// Time in seconds since the last commit on the lane's branch.
    pub branch_freshness_secs: u64,

    /// Current blocker state for this lane.
    pub blocker: LaneBlocker,

    /// Current review status for this lane's diff.
    pub review_status: ReviewStatus,

    /// Scope of the diff associated with this lane.
    pub diff_scope: DiffScope,

    /// Whether lane execution has completed (all DAG nodes executed).
    pub completed: bool,

    /// Whether the lane has been reconciled (no merge needed).
    pub reconciled: bool,
}

/// Blocker state for a lane.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LaneBlocker {
    /// No blocker — lane is free to execute.
    None,

    /// Lane is blocked at startup (e.g., dependency not satisfied).
    Startup,

    /// Lane is blocked by an external factor (e.g., CI failure, external service).
    External,
}

/// Review status for a lane's diff.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewStatus {
    /// Review is pending (not yet reviewed).
    Pending,

    /// Review has been approved.
    Approved,

    /// Review has been rejected.
    Rejected,
}

/// Scope of the diff associated with a lane.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiffScope {
    /// Full repository diff (many files changed).
    Full,

    /// Scoped diff (small number of files changed).
    Scoped,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lane_context_creation() {
        let ctx = LaneContext {
            lane_id: "lane-1".to_string(),
            green_level: 3,
            branch_freshness_secs: 3600,
            blocker: LaneBlocker::None,
            review_status: ReviewStatus::Pending,
            diff_scope: DiffScope::Scoped,
            completed: true,
            reconciled: false,
        };
        assert_eq!(ctx.lane_id, "lane-1");
        assert_eq!(ctx.green_level, 3);
        assert!(ctx.completed);
        assert!(!ctx.reconciled);
    }

    #[test]
    fn test_lane_context_serde_roundtrip() {
        let ctx = LaneContext {
            lane_id: "lane-1".to_string(),
            green_level: 3,
            branch_freshness_secs: 3600,
            blocker: LaneBlocker::Startup,
            review_status: ReviewStatus::Approved,
            diff_scope: DiffScope::Full,
            completed: true,
            reconciled: false,
        };
        let json = serde_json::to_string(&ctx).unwrap();
        let deserialized: LaneContext = serde_json::from_str(&json).unwrap();
        assert_eq!(ctx, deserialized);
    }

    #[test]
    fn test_lane_blocker_serde() {
        assert_eq!(
            serde_json::from_str::<LaneBlocker>("\"none\"").unwrap(),
            LaneBlocker::None
        );
        assert_eq!(
            serde_json::from_str::<LaneBlocker>("\"startup\"").unwrap(),
            LaneBlocker::Startup
        );
        assert_eq!(
            serde_json::from_str::<LaneBlocker>("\"external\"").unwrap(),
            LaneBlocker::External
        );
    }

    #[test]
    fn test_review_status_serde() {
        assert_eq!(
            serde_json::from_str::<ReviewStatus>("\"pending\"").unwrap(),
            ReviewStatus::Pending
        );
        assert_eq!(
            serde_json::from_str::<ReviewStatus>("\"approved\"").unwrap(),
            ReviewStatus::Approved
        );
        assert_eq!(
            serde_json::from_str::<ReviewStatus>("\"rejected\"").unwrap(),
            ReviewStatus::Rejected
        );
    }

    #[test]
    fn test_diff_scope_serde() {
        assert_eq!(
            serde_json::from_str::<DiffScope>("\"full\"").unwrap(),
            DiffScope::Full
        );
        assert_eq!(
            serde_json::from_str::<DiffScope>("\"scoped\"").unwrap(),
            DiffScope::Scoped
        );
    }
}
