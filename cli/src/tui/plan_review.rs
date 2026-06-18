//! Plan review — plan preview state and actions.
//!
//! @canonical .pi/architecture/modules/tui.md#plan-review
//! Implements: Contract Freeze — PlanReview component
//! Issue: issue-tui-contract-freeze
//!
//! # Contract (Frozen)
//!
//! After a user types an intent, the plan preview shows the generated plan
//! and asks for confirmation. Every intent shows the plan first (plan-first
//! principle).
//!
//! # Implementation note
//!
//! The `PlanReviewState` struct below defines the contract schema. The
//! active plan review state is currently stored in [`TuiViewModel`] fields
//! (`intent`, `template_id`, `nodes`, `phase`) and rendered by
//! [`views::plan::render`]. This avoids duplicating state between the
//! ViewModel and a separate PlanReviewState struct while keeping the
//! contract definition visible for reference.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Plan preview state
// ---------------------------------------------------------------------------

/// Action chosen by the user on the plan preview.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlanAction {
    /// Execute the plan with real-time dashboard.
    Run,
    /// Show plan details, return to command bar (no execution).
    PlanOnly,
    /// Save as reusable template.
    Generate,
    /// Cancel, discard plan, return to command bar.
    Cancel,
    /// View diff against previous execution.
    Diff,
    /// Edit parameters.
    Edit,
}

/// A single node in the plan preview.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanNodePreview {
    /// Step number in execution order.
    pub step: u32,
    /// Node name.
    pub name: String,
    /// Tool to execute.
    pub tool: String,
    /// Dependencies (step numbers).
    pub depends_on: Vec<u32>,
    /// Estimated complexity.
    pub complexity: Option<String>,
}

/// Plan preview state shown after intent submission.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PlanReviewState {
    /// Whether the plan preview is currently shown.
    pub active: bool,
    /// The intent that was submitted.
    pub intent: String,
    /// Template ID identified.
    pub template_id: Option<String>,
    /// Confidence score (0.0 — 1.0).
    pub confidence: Option<f64>,
    /// Nodes in the generated plan.
    pub nodes: Vec<PlanNodePreview>,
    /// Parameters extracted from the intent.
    pub parameters: Vec<(String, String)>,
    /// Estimated LLM cost.
    pub llm_cost_estimate: Option<LlmCostEstimate>,
    /// Currently selected node index for navigation.
    pub selected_node: usize,
}

/// Estimated LLM cost for a plan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmCostEstimate {
    /// Estimated number of LLM calls.
    pub estimated_calls: u32,
    /// Estimated total tokens.
    pub estimated_tokens: u32,
}
