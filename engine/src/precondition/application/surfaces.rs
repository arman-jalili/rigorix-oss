//! PreconditionSurfaces — MCP/HTTP error taxonomy for consequential refusals
//! (ADR-017 §Phase A surfaces).
//!
//! @canonical .pi/architecture/modules/precondition.md#surfaces
//! @canonical .pi/architecture/decisions/ADR-017-consequence-gating.md
//! Implements: Contract Freeze — PreconditionSurfaces + structured
//!   `policy_violation` payload
//! Issue: #938 (consequence-gating epic — contract freeze); behavior closed in
//!   ISSUE-CONSEQUENCE-GATING-10 (PreconditionSurfaces)
//!
//! `rigorix_validate_plan` surfaces `precondition_findings[]` pre-run. A runtime
//! refusal maps to a structured `policy_violation`, with the ref in
//! `data.precondition_id` / `data.step` / `data.outcome` — the same shape MCP
//! already uses for sequence-policy and requirement denials. **No new
//! `rigorix.*` method** is introduced; `rigorix.system.version` advertises the
//! capability.
//!
//! # Contract (Frozen)
//! - A refusal is never an opaque internal error: it is a structured
//!   `policy_violation` carrying the precondition id, the step, and the
//!   distinct outcome
//! - `rigorix_validate_plan` returns the redacted findings (no parameter values)
//! - The version capability string is `precondition_gating` (the surfaces
//!   integration reports it)

use serde::{Deserialize, Serialize};

use crate::precondition::domain::{PreconditionFinding, PreconditionOutcome, PreconditionVerdict};

/// The structured payload attached to a `policy_violation` refusal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreconditionPolicyViolation {
    /// Stable id of the precondition that refused.
    pub precondition_id: String,
    /// Name of the refused step.
    pub step: String,
    /// The distinct outcome (`failed` | `error`).
    pub outcome: PreconditionOutcome,
}

impl PreconditionPolicyViolation {
    /// Build the structured refusal payload from a deny verdict.
    ///
    /// Returns `None` for `Dispatch` — a passing verdict is not a violation.
    pub fn from_verdict(verdict: &PreconditionVerdict, step: &str) -> Option<Self> {
        match verdict {
            PreconditionVerdict::Dispatch => None,
            PreconditionVerdict::Deny {
                precondition_id,
                outcome,
            } => Some(Self {
                precondition_id: precondition_id.clone(),
                step: step.to_string(),
                outcome: *outcome,
            }),
        }
    }
}

/// The surfaces contract consumed by the MCP/HTTP hosts.
pub trait PreconditionSurfaces: Send + Sync {
    /// Map a runtime verdict to the structured `policy_violation` payload.
    fn refusal_payload(
        &self,
        verdict: &PreconditionVerdict,
        step: &str,
    ) -> Option<PreconditionPolicyViolation> {
        PreconditionPolicyViolation::from_verdict(verdict, step)
    }

    /// The `rigorix.system.version` capability string this surface advertises.
    fn capability(&self) -> &'static str {
        "precondition_gating"
    }
}

/// Concrete, stateless surfaces adapter.
pub struct PreconditionSurfacesImpl;

impl PreconditionSurfaces for PreconditionSurfacesImpl {}

impl PreconditionSurfacesImpl {
    /// The JSON-RPC `data` object for a structured `policy_violation` refusal
    /// (the shape `rigorix_validate_plan` / runtime refusals return).
    pub fn policy_violation_data(violation: &PreconditionPolicyViolation) -> serde_json::Value {
        serde_json::json!({
            "type": "policy_violation",
            "precondition_id": violation.precondition_id,
            "step": violation.step,
            "outcome": violation.outcome.as_str(),
            "retryable": false,
        })
    }

    /// The redacted `precondition_findings[]` entries for plan validation.
    ///
    /// Carries the evidence fields only — never parameter values or stdout.
    pub fn findings_json(findings: &[PreconditionFinding]) -> Vec<serde_json::Value> {
        findings
            .iter()
            .map(|finding| {
                serde_json::json!({
                    "precondition_id": finding.precondition_id,
                    "step": finding.step,
                    "outcome": finding.outcome.as_str(),
                    "exit_code": finding.exit_code,
                    "inputs_hash": finding.inputs_hash,
                    "checked_at": finding.checked_at.to_rfc3339(),
                    "summary": finding.decision_summary(),
                    "attribution": finding.attribution.map(|reason| reason.as_str()),
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dispatch_yields_no_violation() {
        assert!(
            PreconditionPolicyViolation::from_verdict(&PreconditionVerdict::Dispatch, "pay")
                .is_none()
        );
    }

    #[test]
    fn deny_yields_structured_violation() {
        let verdict = PreconditionVerdict::Deny {
            precondition_id: "beneficiary-eligible".to_string(),
            outcome: PreconditionOutcome::Failed,
        };
        let violation =
            PreconditionPolicyViolation::from_verdict(&verdict, "pay").expect("violation");
        assert_eq!(violation.precondition_id, "beneficiary-eligible");
        assert_eq!(violation.step, "pay");
        assert_eq!(violation.outcome, PreconditionOutcome::Failed);
    }
}
