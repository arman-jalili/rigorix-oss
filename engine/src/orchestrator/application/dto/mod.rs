//! Data Transfer Objects for the Orchestrator module.
//!
//! @canonical .pi/architecture/modules/orchestrator.md#dtos
//! Implements: Contract Freeze — DTO schemas for run, plan_only, cancel, status
//! Issue: #338
//!
//! DTOs define the input/output contracts for service operations.
//! They carry validation metadata and documentation but no behavior.
//!
//! # Contract (Frozen)
//! - Every service operation has a dedicated input and output DTO
//! - DTOs are serializable (JSON for API)
//! - Validation constraints are documented in field docs
//! - Fields use reasonable Rust types (no framework-specific annotations)

use serde::{Deserialize, Serialize};

use crate::identity::domain::IdentityClaim;
use crate::orchestrator::domain::record::{ExecutionRecord, ExecutionStatus};

// ---------------------------------------------------------------------------
// Run DTOs
// ---------------------------------------------------------------------------

/// Input for a full orchestrator run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunInput {
    /// The user's natural-language intent for execution.
    pub intent: String,

    /// Serialized configuration for the run.
    pub config: serde_json::Value,

    /// Repository root path.
    pub repo_root: String,

    /// Repository name for audit (e.g. "my-org/my-repo").
    pub repository: Option<String>,

    /// Author identity for audit (e.g. email or username).
    pub author: Option<String>,

    /// Attributed identity claim for the run author (see identity module).
    /// Supersedes the self-asserted `author` string when present — flows into
    /// the audit envelope's redacted `identity` block.
    #[serde(default)]
    pub identity: Option<IdentityClaim>,

    /// Optional enforcement preset override.
    pub enforcement_preset: Option<String>,
}

/// Output from a full orchestrator run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunOutput {
    /// The execution ID assigned to this run.
    pub execution_id: uuid::Uuid,

    /// The complete execution record with all metadata.
    pub record: ExecutionRecord,

    /// What happened to this run's audit evidence (#1480).
    ///
    /// A run that completed but whose evidence did not is requestable here
    /// instead of inferable from a `tracing::warn!` line. `NotRequested` means
    /// audit was disabled or unwired — the honest state, not a silent `Complete`.
    #[serde(default)]
    pub evidence: EvidenceStatus,

    /// The digest of the receipt this run was admitted under (#1527).
    ///
    /// `None` for a run that declared none — which, until hosts send one, is every run. The value is
    /// the digest `verify` computed, not one the caller supplied, and it is the same string the
    /// audit envelope carries in `receipt_digest`: a **reference**. The receipt's bytes are the
    /// caller's; a copy here would be a second declaration to keep in step with theirs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub receipt_digest: Option<String>,
}

/// The state of a run's audit evidence (#1480).
///
/// Before this existed, the orchestrator dropped the `build_and_send` result
/// under a comment reading "Optional audit (best-effort)", so a run whose
/// envelope was never built, signed, saved or delivered reported exactly what a
/// fully evidenced run reported: success.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum EvidenceStatus {
    /// Audit was disabled, or no audit service is wired.
    #[default]
    NotRequested,

    /// Built, signed, saved to the local trail when one is wired, and delivered
    /// or durably queued.
    Complete,

    /// At least one step of the evidence path did not succeed.
    Degraded {
        /// Which step failed, in the operator's words.
        reason: String,
        /// Envelopes that reached neither the backend nor a durable queue.
        undelivered: u32,
    },
}

// ---------------------------------------------------------------------------
// Plan Only DTOs
// ---------------------------------------------------------------------------

/// Input for a plan-only operation (no execution).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanOnlyInput {
    /// The user's natural-language intent for planning.
    pub intent: String,

    /// Serialized configuration.
    pub config: serde_json::Value,

    /// Repository root path.
    pub repo_root: String,
}

/// Output from a plan-only operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanOnlyOutput {
    /// Plan result from the planning pipeline.
    pub plan: serde_json::Value,

    /// The proposed TaskGraph structure.
    pub graph: serde_json::Value,

    /// Structured sequence-policy findings from R2 plan-time evaluation.
    ///
    /// Populated when a sequence-policy service is configured and a rule
    /// matched a `promote` sequence — surfaced to plan consumers (e.g. MCP
    /// `rigorix_validate_plan`) so the gating decision is visible **before**
    /// a run starts. Empty when no service is configured or no rule matched.
    /// (A matched `deny` rule refuses the plan instead — it is reported as an
    /// `OrchestratorError::SequencePolicyDenied`, never as a silent pass.)
    #[serde(default)]
    pub sequence_findings: Vec<SequencePolicyFinding>,

    /// Structured R9 operator-controlled step-requirement findings from
    /// plan-time evaluation (ADR-015). Populated when a `[[requirements]]`
    /// obligation matched a step — surfaced to plan consumers so the operator
    /// obligation is visible **before** a run starts. Empty when no service is
    /// configured or every matched step satisfies its requirements.
    #[serde(default)]
    pub requirement_findings: Vec<RequirementPolicyFinding>,
}

/// One matched sequence from plan-time evaluation (R2), surfaced to plan
/// preview consumers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SequencePolicyFinding {
    /// Stable id of the rule that matched.
    pub rule_id: String,
    /// Name of the later matched step that the rule gates.
    pub later_step: String,
    /// Action applied to the later step: `"promote"` (the step is built
    /// `requires_approval = true`) or `"deny"`.
    pub action: String,
}

/// One R9 operator-controlled step-requirement outcome (ADR-015), surfaced to
/// plan preview consumers (`rigorix_validate_plan`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequirementPolicyFinding {
    /// Stable id of the requirement that fired.
    pub requirement_id: String,
    /// Name of the matched step that failed the requirement.
    pub step: String,
    /// Action applied: `"deny"` (plan refused) or `"promote"` (step built
    /// `requires_approval = true`).
    pub action: String,
    /// Unmet obligation names — pointer names and/or `"identity"` (never
    /// parameter values).
    pub unmet: Vec<String>,
}

// ---------------------------------------------------------------------------
// Run/Plan From Template DTOs — pre-resolved templates, skip intent→plan pipeline
// ---------------------------------------------------------------------------

/// A single step from a pre-resolved plan template.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateStepDef {
    pub name: String,
    pub tool: String,
    pub description: String,
    pub parameters: serde_json::Value,
    pub requires_approval: bool,
    /// L1 identity gate (F-20260907-05): step refuses to run without an
    /// attested caller identity (idp_token / local_principal).
    #[serde(default)]
    pub require_identity: bool,
    pub timeout_secs: Option<u64>,
    /// Whether to run scored evaluation on this step's output.
    #[serde(default)]
    pub evaluate_score: bool,
    /// Steps this one depends on, by `name` (#1526).
    ///
    /// `serde(default)`: every existing caller sends no such field, and a plan that declares none
    /// keeps the frozen sequential contract — each step depends on its predecessor. A plan that
    /// declares at least one becomes dependency-driven, and then a step with no dependencies has
    /// none: it may run first. The names are resolved, and the four ways a set can be wrong are
    /// refused, by `orchestrator::domain::step_graph`.
    #[serde(default)]
    pub depends_on: Vec<String>,
}

/// Input for a full orchestrator run from a pre-resolved template.
///
/// Skips the planning pipeline (intent→classify→match→generate) because
/// the steps are already concrete. Everything else — state persistence,
/// DAG execution, quality gates, policy engine, audit dispatch — is
/// identical to a normal `run()`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunFromTemplateInput {
    /// Pre-resolved steps to execute.
    pub steps: Vec<TemplateStepDef>,

    /// Repository root path.
    pub repo_root: String,

    /// Optional execution ID (generated if not provided).
    pub execution_id: Option<uuid::Uuid>,

    /// Template name for audit and record metadata.
    pub template_name: String,

    /// Repository name for audit (e.g. "my-org/my-repo").
    pub repository: Option<String>,

    /// Author identity for audit (e.g. email or username).
    pub author: Option<String>,

    /// Attested caller identity (L1 identity gate / L2 principal hygiene,
    /// F-20260907-05). When present, steps declaring `require_identity` may
    /// run, and the SEQUENCE-POLICY principal is this claim's subject (the
    /// caller-supplied `author` stays display-only for policy purposes).
    /// `None` or source = unverified → `require_identity` steps are refused
    /// at plan time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub identity: Option<crate::identity::domain::IdentityRef>,

    /// The identity provider's recorded failure, supplied by the process that
    /// performed verification (#1285).
    ///
    /// The engine cannot know which stage of verification failed — discovery or
    /// the JWKS fetch — so it does not guess: when the caller's verifier
    /// recorded a cause, the refusal quotes it and names no stage. Request-side
    /// only: this never enters an audit envelope (`IdentityRef` carries the
    /// claim and stays frozen, ADR-016).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub identity_verification_failure: Option<String>,

    /// Optional enforcement preset override.
    pub enforcement_preset: Option<String>,

    /// The declaration this run is asked to satisfy (#1527, ADR-024 §4).
    ///
    /// Verified **before** the identity gate, the policy pipeline, any state write and any graph: a
    /// refused receipt returns [`OrchestratorError::ReceiptRefused`](crate::orchestrator::domain::error::OrchestratorError::ReceiptRefused)
    /// and nothing is dispatched, so "the receipt was refused" and "no step ran" are the same
    /// statement rather than two claims a reader has to correlate.
    ///
    /// `None` means the caller declared nothing, which is what every caller did before this field
    /// existed. A verified receipt's digest reaches [`RunOutput::receipt_digest`] and the audit
    /// envelope; its steps are **not** yet the plan (see the PR: P2 verifies the declaration, it does
    /// not derive the runbook from it).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub receipt: Option<crate::receipt::domain::receipt::Receipt>,
}

/// Input for a plan-from-template operation (no execution).
///
/// Builds a TaskGraph from pre-resolved steps and returns the graph
/// structure without executing anything. Used for preview/validation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanFromTemplateInput {
    /// Pre-resolved steps.
    pub steps: Vec<TemplateStepDef>,

    /// Repository root path.
    pub repo_root: String,

    /// Template name for metadata.
    pub template_name: String,

    /// Principal (author/attested identity) — lets the R2 gate evaluate R7
    /// same-principal history rules during plan preview/validation.
    pub author: Option<String>,

    /// Attested caller identity — same semantics as `RunFromTemplateInput`:
    /// `require_identity` steps refused at preview when absent/unverified;
    /// sequence-policy principal prefers this claim's subject.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub identity: Option<crate::identity::domain::IdentityRef>,

    /// The identity provider's recorded failure, supplied by the process that
    /// performed verification (#1285).
    ///
    /// The engine cannot know which stage of verification failed — discovery or
    /// the JWKS fetch — so it does not guess: when the caller's verifier
    /// recorded a cause, the refusal quotes it and names no stage. Request-side
    /// only: this never enters an audit envelope (`IdentityRef` carries the
    /// claim and stays frozen, ADR-016).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub identity_verification_failure: Option<String>,
}

// ---------------------------------------------------------------------------
// Cancel DTOs
// ---------------------------------------------------------------------------

/// Input for cancelling a running execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CancelInput {
    /// The execution ID to cancel.
    pub execution_id: uuid::Uuid,

    /// Optional reason for cancellation.
    pub reason: Option<String>,
}

/// Output from a cancel operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CancelOutput {
    /// The execution ID that was cancelled.
    pub execution_id: uuid::Uuid,

    /// Whether the execution was successfully aborted.
    pub aborted: bool,

    /// How many DAG nodes were cancelled mid-execution.
    pub nodes_cancelled: u32,
}

// ---------------------------------------------------------------------------
// Approval DTOs
// ---------------------------------------------------------------------------

/// Input for approving steps of a paused execution.
///
/// Grants human sign-off for steps that declared `requires_approval: true`
/// and resumes the paused execution once approval is recorded.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApproveExecutionInput {
    /// The execution ID to approve steps for.
    pub execution_id: uuid::Uuid,

    /// Step (node) names to approve.
    pub step_names: Vec<String>,

    // ── ADR-011 approval binding (optional) ───────────────────────────────
    // With the binding enabled the approving identity is a required captured
    // fact (R3). When omitted the orchestrator falls back to the run's own
    // author identity if one was recorded; otherwise the engine denies.
    /// Human identity subject approving (see identity module).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approver_id: Option<String>,
    /// Role / policy id (captured fact, not a judgment).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub authority: Option<String>,
    /// IdP token/claims presented at approval (credential-substitution check).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token_claims_ref: Option<String>,
}

/// Output from approving steps of a paused execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApproveExecutionOutput {
    /// The execution ID.
    pub execution_id: uuid::Uuid,

    /// Step names that were approved.
    pub approved: Vec<String>,

    /// Step names that could not be found in this execution.
    pub not_found: Vec<String>,

    /// Step names still awaiting approval after this call.
    pub still_pending: Vec<String>,

    /// Whether the paused execution resumed after approval.
    pub resumed: bool,

    /// Steps the approval gate refused, with the reason and remediation
    /// (#1235). Empty when every requested step was approved.
    #[serde(default)]
    pub denied_reasons: Vec<crate::execution_engine::application::dto::ApprovalDenial>,
}

// ---------------------------------------------------------------------------
// Status DTOs
// ---------------------------------------------------------------------------

/// Output from a status query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusOutput {
    /// The current execution ID.
    pub execution_id: uuid::Uuid,

    /// Current execution status.
    pub status: ExecutionStatus,

    /// Per-node state information.
    pub nodes: Vec<NodeState>,
}

/// State of a single DAG node from a status query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeState {
    /// Unique identifier of the DAG node.
    pub node_id: String,

    /// Human-readable node name.
    pub node_name: String,

    /// Current status of this node.
    pub status: String,

    /// Human-readable status message.
    pub message: Option<String>,
}
