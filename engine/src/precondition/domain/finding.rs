//! PreconditionFinding — the signed-envelope evidence shape and the canonical
//! `PreconditionChecked` event payload (ADR-017 §R1 evidence).
//!
//! @canonical .pi/architecture/modules/precondition.md#evidence
//! @canonical .pi/architecture/modules/precondition.md#fail-modes
//! @canonical .pi/architecture/decisions/ADR-017-consequence-gating.md
//! Implements: Contract Freeze — PreconditionOutcome, PreconditionFinding,
//!   PreconditionChecked
//! Issue: #938 (consequence-gating epic — contract freeze); behavior closed in
//!   ISSUE-CONSEQUENCE-GATING-8 (PreconditionFinding)
//!
//! The envelope field `precondition_findings[]` and the `PreconditionChecked`
//! event are **additive**: absent/empty in pre-consequence-gating envelopes, so
//! existing signed bytes are unchanged (`skip_serializing_if = "Vec::is_empty"`
//! / `Option::is_none` at the integration seam).
//!
//! # Contract (Frozen)
//! - Outcome is one of `passed` / `failed` / `error`; `failed` and `error`
//!   both **refuse**, and are recorded **distinctly** for operability
//! - A finding records `precondition_id`, `step`, `outcome`, `exit_code`
//!   (when a process ran), `inputs_hash` (one-way hash of the inputs — never
//!   the values), `checked_at`, and a redacted `summary`
//! - No parameter VALUES and no stdout are recorded by default (SpanPrivacy);
//!   stdout is opt-in via `capture_output` and never leaves the local store
//! - `decision_summary()` is the redaction boundary: it names the check, the
//!   step, and the outcome — never inputs or output

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// The distinct result of running a precondition check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PreconditionOutcome {
    /// Exit 0 — the authority stands; the step may dispatch.
    Passed,
    /// Non-zero exit — the operator's check explicitly denied. Refuses.
    Failed,
    /// Timeout, spawn failure, malformed match, or trust-boundary violation —
    /// indeterminate. Refuses, and is recorded distinctly from `failed`.
    Error,
}

impl PreconditionOutcome {
    /// Canonical lowercase wire string (`"passed" | "failed" | "error"`).
    pub fn as_str(&self) -> &'static str {
        match self {
            PreconditionOutcome::Passed => "passed",
            PreconditionOutcome::Failed => "failed",
            PreconditionOutcome::Error => "error",
        }
    }

    /// Whether this outcome refuses the matched step. `passed` dispatches;
    /// `failed` and `error` refuse (ADR-017 §R1 — no allow-on-error mode).
    pub fn refuses(&self) -> bool {
        !matches!(self, PreconditionOutcome::Passed)
    }
}

/// Structured reason the ADR-017 attribution digests are present or absent
/// (ISSUE-ATTRIBUTION-ABSENCE-REASON).
///
/// Before this, an absent `check_digest` / `authority_digest` meant three
/// different things; a consumer could not tell a boundary refusal (the
/// interesting case) from an old record. New engines always set this, so
/// absence itself means "written by a pre-attribution engine".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttributionReason {
    /// The check ran (passed or failed) and attribution was collected.
    Recorded,
    /// A trust-boundary / boundary-strength refusal, a missing required
    /// parameter, or an unarmed gate — nothing was hashed or executed. The
    /// digests are omitted; the summary carries the reason.
    RefusedBeforeCheck,
    /// The check (or its authority) existed but a digest could not be computed
    /// (unreadable artifact / permissions / race). A weaker signal than a
    /// clean refusal; any digest that *could* be computed is still present.
    ArtifactUnreadable,
}

impl AttributionReason {
    /// Canonical lowercase wire string
    /// (`"recorded" | "refused_before_check" | "artifact_unreadable"`).
    pub fn as_str(&self) -> &'static str {
        match self {
            AttributionReason::Recorded => "recorded",
            AttributionReason::RefusedBeforeCheck => "refused_before_check",
            AttributionReason::ArtifactUnreadable => "artifact_unreadable",
        }
    }
}

/// A recorded, redacted precondition determination for one matched step.
///
/// Summary fields only — the precondition id, the step, the outcome, the exit
/// code, a one-way inputs hash, and the timestamp. Parameter **values** and raw
/// stdout never appear (SpanPrivacy).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreconditionFinding {
    /// Stable id of the precondition that ran.
    pub precondition_id: String,
    /// Name of the matched step the precondition gated.
    pub step: String,
    /// The distinct outcome.
    pub outcome: PreconditionOutcome,
    /// Process exit code, when a process actually ran. `None` for a
    /// pre-execution refusal (e.g. missing `require_params`, `unarmed`,
    /// trust-boundary) and for timeout / spawn failure.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    /// One-way hash of the check inputs (never the inputs themselves).
    pub inputs_hash: String,
    /// When the check was evaluated.
    pub checked_at: DateTime<Utc>,
    /// Redacted decision summary (never parameter values or stdout).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    /// SHA-256 (`sha256:<hex>`) of the resolved check program bytes the
    /// determination is attributed to (#987). Omitted when the program could
    /// not be read. Never its contents.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub check_digest: Option<String>,
    /// SHA-256 (`sha256:<hex>`) of the operator-declared authority artifact the
    /// check consulted (#987). Omitted when no `authority_path` was declared or
    /// it could not be read. Detection, not prevention: a changed authority
    /// changes this digest.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub authority_digest: Option<String>,
    /// Boundary fact (#986): whether the resolved check was writable by the
    /// engine's effective UID. `None` when unknown.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub check_writable: Option<bool>,
    /// Structured attribution reason (ISSUE-ATTRIBUTION-ABSENCE-REASON).
    /// Additive and optional: absent ⇒ a pre-attribution engine; a new engine
    /// always sets it. Serialized last to keep existing canonical bytes stable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attribution: Option<AttributionReason>,
}

impl PreconditionFinding {
    /// Redacted one-line summary — parameter values and stdout are never
    /// included.
    pub fn decision_summary(&self) -> String {
        if let Some(summary) = &self.summary {
            return summary.clone();
        }
        match self.exit_code {
            Some(code) => format!(
                "precondition '{}' {} step '{}' (exit {code})",
                self.precondition_id,
                self.outcome.as_str(),
                self.step
            ),
            None => format!(
                "precondition '{}' {} step '{}'",
                self.precondition_id,
                self.outcome.as_str(),
                self.step
            ),
        }
    }
}

/// The canonical `PreconditionChecked` event payload.
///
/// The event-system integration is additive: `ExecutionEvent::PreconditionChecked`
/// carries exactly these fields (plus the standard `timestamp`), and the audit
/// factory derives `precondition_findings[]` from it. The payload is frozen
/// here so both integrations consume one schema.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreconditionChecked {
    /// Globally unique execution identifier.
    pub execution_id: uuid::Uuid,
    /// Stable id of the precondition that was evaluated.
    pub precondition_id: String,
    /// Name of the matched (gated) step.
    pub step: String,
    /// The distinct outcome.
    pub outcome: PreconditionOutcome,
    /// Process exit code, when a process ran.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    /// One-way hash of the check inputs (never the inputs).
    pub inputs_hash: String,
    /// Redacted decision summary.
    pub summary: String,
    /// SHA-256 (`sha256:<hex>`) of the check program (ADR-017 attribution).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub check_digest: Option<String>,
    /// SHA-256 (`sha256:<hex>`) of the authority artifact consulted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub authority_digest: Option<String>,
    /// Boundary fact: the check was writable by the engine's euid.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub check_writable: Option<bool>,
    /// Structured attribution reason (ISSUE-ATTRIBUTION-ABSENCE-REASON).
    /// Wire string: `recorded` | `refused_before_check` | `artifact_unreadable`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attribution: Option<String>,
    /// ISO 8601 timestamp of the event.
    pub timestamp: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn finding(outcome: PreconditionOutcome, exit_code: Option<i32>) -> PreconditionFinding {
        PreconditionFinding {
            precondition_id: "beneficiary-eligible".to_string(),
            step: "pay".to_string(),
            outcome,
            exit_code,
            inputs_hash: "sha256:deadbeef".to_string(),
            checked_at: Utc::now(),
            summary: None,
            check_digest: None,
            authority_digest: None,
            check_writable: None,
            attribution: None,
        }
    }

    #[test]
    fn passed_dispatches_and_failed_and_error_refuse() {
        assert!(!PreconditionOutcome::Passed.refuses());
        assert!(PreconditionOutcome::Failed.refuses());
        assert!(PreconditionOutcome::Error.refuses());
    }

    #[test]
    fn outcome_wire_format_is_lowercase_snake() {
        assert_eq!(
            serde_json::to_value(PreconditionOutcome::Passed).unwrap(),
            serde_json::json!("passed")
        );
        assert_eq!(
            serde_json::to_value(PreconditionOutcome::Failed).unwrap(),
            serde_json::json!("failed")
        );
        assert_eq!(
            serde_json::to_value(PreconditionOutcome::Error).unwrap(),
            serde_json::json!("error")
        );
    }

    #[test]
    fn decision_summary_never_carries_values() {
        let summary = finding(PreconditionOutcome::Failed, Some(3)).decision_summary();
        assert!(summary.contains("beneficiary-eligible"));
        assert!(summary.contains("failed"));
        assert!(summary.contains("exit 3"));
    }

    #[test]
    fn failed_and_error_round_trip_distinctly() {
        let failed = finding(PreconditionOutcome::Failed, Some(1));
        let error = finding(PreconditionOutcome::Error, None);
        assert_ne!(failed.outcome, error.outcome);
        let encoded = serde_json::to_string(&failed).expect("serialize");
        let decoded: PreconditionFinding = serde_json::from_str(&encoded).expect("deserialize");
        assert_eq!(decoded, failed);
    }
}
