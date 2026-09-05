//! SequenceRule — one declarative rule over an ordered step sequence.
//!
//! @canonical .pi/architecture/modules/sequence-policy.md#sequencerule
//! Implements: Contract Freeze — SequenceRule, StepPredicate, ParamPredicate,
//!   ParamMatchKind, RuleAction
//! Issue: #838 (sequence-policy epic — contract freeze); behavior in
//!   ISSUE-SEQUENCE-POLICY-1 (SequenceRule) and ISSUE-SEQUENCE-POLICY-4
//!   (StepPredicate)
//!
//! A rule describes an **ordered pair (or windowed chain)** of step
//! predicates. Predicates match on `tool` (exact or glob) and optionally on
//! parameter values (JSON pointer + exact/glob/regex value predicate). Rules
//! carry an `action`:
//!
//! | Action | Semantics |
//! |--------|-----------|
//! | `promote` (default) | Later matched step becomes `requires_approval = true` — human decides |
//! | `deny` | Later matched step is denied before dispatch (structured `SequencePolicyDenied` node failure) |
//!
//! No rule may reference LLM output, model intent, or conversation content —
//! matching is over the execution plan/prefix only (R1, R4).
//!
//! # Contract (Frozen)
//! - Serde round-trip preserves every field (`id`, `name`, `description`,
//!   ordered `steps`, optional `window`, `action`)
//! - Wire kinds serialize lowercase: `kind = "exact" | "glob" | "regex"`,
//!   `action = "promote" | "deny"`
//! - `action` defaults to `promote` when omitted; `params` defaults to empty
//!   when omitted (backward-compatible config evolution)
//! - Matching *behavior* (tool glob, param exact/glob/regex) is implemented in
//!   the ISSUE-SEQUENCE-POLICY-1 / ISSUE-SEQUENCE-POLICY-4 / -5 issues — the
//!   data contract is frozen here

use serde::{Deserialize, Serialize};

/// Action taken on the later matched step of a rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RuleAction {
    /// Later matched step is built `requires_approval = true` — the existing
    /// approval pause/resume chain decides (default).
    #[default]
    Promote,
    /// Later matched step is denied before dispatch (`SequencePolicyDenied`).
    Deny,
}

/// Kind of value comparison applied by a `ParamPredicate`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ParamMatchKind {
    /// Exact string equality.
    Exact,
    /// Glob match over the value (e.g. `conf-*`).
    Glob,
    /// Regular-expression match over the value.
    Regex,
}

/// One parameter predicate: a JSON pointer into the step's parameter object
/// plus a value predicate (exact / glob / regex).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParamPredicate {
    /// JSON pointer into the step parameters, e.g. `"/event_id"`.
    pub pointer: String,
    /// How `value` is compared (exact | glob | regex).
    pub kind: ParamMatchKind,
    /// Expected value string for the predicate.
    pub value: String,
}

/// A step predicate: matches a tool name (exact or glob) plus optional
/// parameter predicates.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StepPredicate {
    /// Tool name — exact or glob, e.g. `"registration_remove"`,
    /// `"registration_*"`.
    pub tool: String,
    /// Optional parameter predicates; empty means "any parameters".
    #[serde(default)]
    pub params: Vec<ParamPredicate>,
}

/// One declarative rule over an ordered step sequence.
///
/// Example (the conference case from the module spec):
///
/// ```toml
/// [[rules]]
/// id = "registration-remove-then-reassign"
/// name = "No remove-then-reassign of a full event seat"
/// description = "Removing an attendee to free a seat, then registering the requester, is never autonomous"
/// steps = [
///   { tool = "registration_remove", params = [{ pointer = "/event_id", kind = "exact", value = "conf-2026" }] },
///   { tool = "registration_add",    params = [{ pointer = "/event_id", kind = "exact", value = "conf-2026" }] },
/// ]
/// window = 3
/// action = "promote"   # the add step pauses for a human
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SequenceRule {
    /// Stable identifier, e.g. `"registration-remove-then-reassign"`.
    pub id: String,
    /// Human-readable rule name.
    pub name: String,
    /// Why the rule exists (surfaced to approvers / audit summaries).
    pub description: String,
    /// Ordered step predicates — a match requires `steps[i]` to appear before
    /// `steps[i + 1]` in the evaluated ordered step list.
    pub steps: Vec<StepPredicate>,
    /// Maximum index gap between the first and last matched step
    /// (default: adjacent pair, gap of 1).
    pub window: Option<u32>,
    /// Action on match: `promote` (default) or `deny`.
    #[serde(default)]
    pub action: RuleAction,
}
