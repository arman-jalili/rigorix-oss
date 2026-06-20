//! Event payload schemas for the Policy Evaluator bounded context.
//!
//! @canonical actions/.pi/architecture/modules/policy-evaluator.md
//! Implements: Contract Freeze — PolicyEvent payload schemas
//! Issue: issue-contract-freeze
//!
//! These events are emitted on the EventBus whenever a policy is loaded,
//! evaluated, or merged with org-level policies. Consumers (audit, CI
//! integration, reporting) subscribe to these event types.
//!
//! # Contract (Frozen)
//! - Each event carries the full context needed by consumers
//! - No internal implementation details exposed
//! - Events are serializable for audit logging

use serde::{Deserialize, Serialize};

use super::types::{PolicyDocument, PolicyResult, PolicyViolation};

/// Events emitted by the Policy Evaluator module.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PolicyEvent {
    /// Policy document was successfully loaded from a repository.
    PolicyLoaded {
        /// The loaded policy document.
        policy: PolicyDocument,
        /// The source reference (branch/commit) from which it was loaded.
        source_ref: String,
        /// The policy file path.
        path: String,
        /// Whether the policy was loaded from the base branch.
        from_base_branch: bool,
    },

    /// Organization policy was successfully loaded and merged.
    OrgPolicyMerged {
        /// The merged policy document after organization merge.
        merged_policy: PolicyDocument,
        /// Whether org rules were added during merge.
        org_rules_added: bool,
        /// Whether limits were tightened by the org policy.
        limits_tightened: bool,
    },

    /// Policy tampering was detected (PR modifies the policy file).
    PolicyTamperDetected {
        /// The policy file path that was modified.
        path: String,
        /// Whether the action will proceed despite the tampering.
        proceeding: bool,
    },

    /// Policy evaluation completed against a PR diff.
    PolicyEvaluated {
        /// The evaluation result.
        result: PolicyResult,
        /// Number of files evaluated.
        files_evaluated: usize,
        /// Number of violations found.
        violation_count: usize,
        /// Whether the evaluation resulted in a blocking action.
        is_blocking: bool,
        /// Evaluation duration in milliseconds.
        evaluation_time_ms: u64,
    },

    /// A specific violation was detected.
    ViolationDetected {
        /// The violation details.
        violation: PolicyViolation,
        /// The violation index in the results list.
        index: usize,
        /// Whether this is the first violation in this category.
        first_of_type: bool,
    },

    /// A policy rule matched a file (informational, not a violation).
    RuleMatched {
        /// The rule name that matched.
        rule_name: String,
        /// The rule type (deny, review, flag).
        rule_type: String,
        /// The file path that matched.
        file: String,
    },

    /// Policy evaluation failed with an error.
    PolicyError {
        /// The error message.
        error: String,
        /// Whether the error is blocking the action.
        is_blocking: bool,
        /// Whether the error is retriable.
        is_retriable: bool,
    },

    /// Full policy evaluation pipeline completed.
    EvaluationCompleted {
        /// The final evaluation result.
        result: PolicyResult,
        /// Total processing time in milliseconds.
        processing_time_ms: u64,
        /// Summary of the evaluation.
        summary: EvaluationSummary,
    },
}

/// Summary of a full policy evaluation pipeline execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluationSummary {
    /// Whether the policy was loaded successfully.
    pub policy_loaded: bool,
    /// Whether organization policy was merged.
    pub org_policy_merged: bool,
    /// Number of deny violations found.
    pub deny_violations: usize,
    /// Number of review violations found.
    pub review_violations: usize,
    /// Number of flag violations found.
    pub flag_violations: usize,
    /// Whether policy tampering was detected.
    pub tamper_detected: bool,
    /// Number of files evaluated.
    pub files_evaluated: usize,
}
