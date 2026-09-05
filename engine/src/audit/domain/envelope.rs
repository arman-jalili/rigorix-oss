//! AuditEnvelope domain entity.
//!
//! @canonical .pi/architecture/modules/audit.md#envelope
//! Implements: Contract Freeze — AuditEnvelope value object with execution metadata
//! Issue: #13
//!
//! Typed envelope containing execution audit data. Carries execution metadata,
//! planning hash for replay verification, and an optional HMAC signature for
//! integrity protection.
//!
//! # Contract (Frozen)
//! - `AuditEnvelope` is the value object for all audit records
//! - All fields are public for direct construction by the application layer
//! - Construction happens via `AuditEnvelopeFactory`
//! - Signature is optional — populated when HMAC signing is configured

use serde::{Deserialize, Serialize};

use crate::identity::domain::IdentityRef;

/// Typed envelope containing execution audit data.
///
/// Built at execution completion by the orchestration layer and sent to
/// the configured audit backend via `AuditSender`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEnvelope {
    /// Globally unique execution identifier (UUID v4).
    pub execution_id: uuid::Uuid,

    /// Timestamp when the execution completed (ISO 8601 / UTC).
    pub timestamp: chrono::DateTime<chrono::Utc>,

    /// Identifier of the template that generated this execution plan.
    pub template_id: String,

    /// Hash of the planning prompt used for replay reproducibility.
    ///
    /// Allows verifying that the same input produces the same plan.
    pub planning_hash: String,

    /// Source/environment that triggered this execution
    /// (e.g. "rigorix_action", "rigorix_cli").
    pub source: Option<String>,

    /// Repository the execution ran against (e.g. "my-org/my-repo").
    pub repository: Option<String>,

    /// Identity of the user or bot that triggered the execution.
    pub author: Option<String>,

    /// Attributed human identity of author/approver (see identity module).
    ///
    /// Redacted summary — subject/issuer/source/authority/expiry — never the
    /// raw token (`.pi/architecture/modules/audit.md` identity block).
    /// Additive and serde-defaulted: absent in pre-identity envelopes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub identity: Option<IdentityRef>,

    /// Total number of LLM tokens consumed during this execution.
    ///
    /// Used for cost estimation and AI ROI analytics in the Enterprise dashboard.
    pub total_tokens: u32,

    /// Total wall-clock duration of the execution in milliseconds.
    ///
    /// Used for performance analytics and time-based cost estimation.
    pub duration_ms: u64,

    /// Git commit hash of the repository at the time of execution.
    ///
    /// Used for compliance provenance chains.
    pub git_commit: Option<String>,

    /// Git branch name at the time of execution.
    ///
    /// Used for compliance provenance chains.
    pub git_branch: Option<String>,

    /// LLM model version used for planning (e.g. "claude-sonnet-4-20250514").
    /// `None` when no LLM was used or the model version was not captured.
    pub model_version: Option<String>,

    /// The planning prompt text (opt-in, privacy-sensitive).
    ///
    /// Only populated when prompt content capture is enabled in configuration.
    /// Used for compliance provenance and audit evidence.
    pub planning_prompt: Option<String>,

    /// File paths changed or created during this execution (if available).
    ///
    /// Used for compliance provenance chains to track which files were
    /// modified by AI-generated output.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub file_paths: Vec<String>,

    /// The ordered list of execution events captured during this run.
    pub events: Vec<ExecutionEventRef>,

    /// Scoring results from scored_evaluation nodes, keyed by node_id.
    ///
    /// Populated when scored evaluation nodes are present in the DAG.
    /// Used for compliance provenance and quality audit evidence.
    #[serde(default, skip_serializing_if = "std::collections::HashMap::is_empty")]
    pub scoring_results: std::collections::HashMap<String, ScoringResultRef>,

    /// HMAC signature for envelope integrity verification.
    ///
    /// `None` if HMAC signing is not configured.
    pub signature: Option<String>,

    /// Evidence-integrity marker: true when this envelope was produced
    /// WITHOUT an HMAC signature (signing not requested). Approval-bearing
    /// runs must be signed; unsigned envelopes carry this explicit degraded
    /// marker so consumers can distinguish "intentionally unsigned" from
    /// "tampered" (GAP-M-12).
    #[serde(default)]
    pub evidence_degraded: bool,

    /// Signed approval decisions, in approval order (ADR-011 R3).
    ///
    /// Additive and serde-defaulted: absent in pre-approval envelopes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub approval_events: Vec<ApprovalRecordRef>,

    /// Post-execution scope violations — non-blocking evidence (ADR-011 R5).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scope_violations: Vec<ScopeViolationRef>,

    /// Sequence-policy decisions recorded during the run (R6): every matched
    /// rule — promoted later step or dispatch-boundary deny — as a redacted
    /// finding. Answers "why did the run pause / why was this step denied"
    /// from the signed record.
    ///
    /// Additive and serde-defaulted: absent in pre-sequence-policy envelopes.
    /// Summaries redact parameter values by default (SpanPrivacy pattern);
    /// full payloads are opt-in and never leave the local store.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sequence_policy_findings: Vec<SequencePolicyFindingRef>,

    /// Reference + summary of the decision context shown to the approver;
    /// the full payload is opt-in and stored locally (R4 privacy pattern).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decision_context_ref: Option<String>,
}

/// A signed reference to a human approval decision (ADR-011 R3).
///
/// Summary fields only — the full record and decision payload live in the
/// local store; the envelope proves the decision happened and what it bound.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalRecordRef {
    /// Node id that was approved.
    pub node_id: uuid::Uuid,
    /// Human step name.
    pub step_name: String,
    /// Intent hash the approval is bound to.
    pub intent_hash: String,
    /// Identity subject who approved (captured fact).
    pub approver_id: String,
    /// Role / policy id (captured fact).
    pub authority: Option<String>,
    /// When the human approved.
    pub decided_at: chrono::DateTime<chrono::Utc>,
}

/// A redacted reference to a sequence-policy decision (R6).
///
/// Summary fields only — the rule id, action taken, the concrete matched
/// step indices, and a decision summary that NEVER carries step parameter
/// values (SpanPrivacy default; the full payload is opt-in and local).
/// Derived from `sequence_rule_matched` / `sequence_policy_denied` events
/// at envelope build time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SequencePolicyFindingRef {
    /// Stable id of the rule that matched.
    pub rule_id: String,
    /// Action taken: `"promote"` (later step paused for a human) or
    /// `"deny"` (step denied before dispatch).
    pub action: String,
    /// Name of the later matched step — the step the rule gated.
    pub later_step: String,
    /// Indices (into the evaluated ordered step list) of the matched steps.
    pub matched_indices: Vec<usize>,
    /// Redacted decision summary (parameter values never included).
    pub summary: String,
}

/// A reference to a recorded effect-scope violation (ADR-011 R5).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScopeViolationRef {
    /// Node id whose execution produced the violation.
    pub node_id: uuid::Uuid,
    /// Human step name.
    pub step_name: String,
    /// Effects outside the declared scope (from the git-diff oracle).
    pub out_of_scope: Vec<String>,
    /// When the violation was detected.
    pub detected_at: chrono::DateTime<chrono::Utc>,
}

/// A reference to a scoring result included in the audit envelope.
///
/// Contains the pass/fail status, backend metadata, and dimension scores
/// but not the full raw response (to keep envelope size manageable).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoringResultRef {
    /// Whether all scoring dimensions passed.
    pub passed: bool,

    /// Name of the scoring backend that produced this result.
    pub backend: String,

    /// Map of dimension name to score dimension reference.
    pub dimensions: std::collections::HashMap<String, ScoreDimensionRef>,

    /// Duration of the evaluation in milliseconds.
    pub duration_ms: u64,
}

/// A reference to a single scoring dimension within a scoring result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoreDimensionRef {
    /// The achieved score (0.0–1.0).
    pub score: f64,

    /// The maximum possible score.
    pub max: f64,

    /// Human-readable label for this dimension.
    pub label: String,

    /// Whether this dimension passed.
    pub passed: bool,
}

/// A reference to an execution event included in the audit envelope.
///
/// Contains the event type, timestamp, and a correlation identifier
/// but not the full event payload (to keep envelope size manageable).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionEventRef {
    /// Machine-readable event type (e.g. "task_completed", "tool_executed").
    pub event_type: String,

    /// Human-readable event summary for audit review.
    pub summary: String,

    /// ISO 8601 timestamp of when the event occurred.
    pub occurred_at: chrono::DateTime<chrono::Utc>,

    /// Correlation ID linking this event across services.
    pub correlation_id: Option<uuid::Uuid>,

    /// Whether this event represents a success or failure.
    pub status: EventStatus,

    /// Optional JSON payload with event-specific details.
    ///
    /// May contain per-node output, tool usage data, budget warning details,
    /// or error information depending on the event type.
    /// Omitted when empty to keep envelope size manageable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payload: Option<serde_json::Value>,
}

/// Status of an execution event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EventStatus {
    /// Event completed successfully.
    Success,
    /// Event failed with an error.
    Failure,
    /// Event was skipped (e.g. due to conditionals).
    Skipped,
    /// Event was cancelled.
    Cancelled,
}

/// Circuit breaker state machine for resilient HTTP delivery.
///
/// Follows the standard closed → open → half-open → closed pattern.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CircuitBreakerState {
    /// Normal operation — requests pass through.
    Closed,
    /// Failure threshold exceeded — requests are rejected immediately.
    Open,
    /// Probing — a single test request is allowed through.
    HalfOpen,
}
