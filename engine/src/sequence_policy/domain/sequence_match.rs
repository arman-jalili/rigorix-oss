//! SequenceMatch — the matched-window result returned by plan-time (R2) and
//! run-time prefix (R3) evaluation.
//!
//! @canonical .pi/architecture/modules/sequence-policy.md#r2--plan-time-evaluation-primary-pre-side-effect
//! Implements: Contract Freeze — SequenceMatch
//! Issue: #838 (sequence-policy epic — contract freeze); matching behavior in
//!   ISSUE-SEQUENCE-POLICY-5 (Matcher)
//!
//! A `SequenceMatch` names the rule that matched, the action to take, the
//! positions of the concrete steps that satisfied the rule's ordered
//! predicates, and the *later matched step* — the step that must be promoted
//! (`requires_approval = true`) or denied.
//!
//! # Contract (Frozen)
//! - `rule_id` identifies the matched rule (stable identifier from
//!   `.rigorix/sequence-policy.toml`)
//! - `action` is denormalized onto the match so the orchestrator /
//!   execution-engine choke point can branch without re-reading the rule set:
//!   `Promote` → approval pause path, `Deny` → `SequencePolicyDenied` failure
//! - `matched_indices` are the indices (into the evaluated ordered step list)
//!   of the concrete steps that satisfied the rule's predicates, in order
//! - `later_step` is the name of the later matched step — the step the engine
//!   promotes or denies
//! - The type is serializable: matches are recorded verbatim into the audit
//!   event (`SequenceRuleMatched { rule_id, action, later_step }`) and the
//!   envelope `sequence_policy_findings[]` (R6)

use serde::{Deserialize, Serialize};

use super::rule::RuleAction;

/// A matched window within a plan or dispatch prefix.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SequenceMatch {
    /// Stable id of the rule that matched (e.g.
    /// `"registration-remove-then-reassign"`).
    pub rule_id: String,
    /// Action the engine must take on the later matched step: promote to
    /// approval, or deny before dispatch.
    pub action: RuleAction,
    /// Indices (into the evaluated ordered step list) of the concrete steps
    /// that satisfied the rule's ordered predicates, in rule order.
    pub matched_indices: Vec<usize>,
    /// Name of the later matched step — the step that is promoted
    /// (`requires_approval = true`) or denied.
    pub later_step: String,
}
