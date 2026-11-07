//! The finding: one gate's conclusion about one step, in evaluation order.
//!
//! @canonical .pi/architecture/modules/gate.md

use serde::{Deserialize, Serialize};

use super::outcome::GateOutcome;

/// Which gate produced a finding, by the module that owns it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GateId {
    /// `precondition` — the dispatch-time gate (ADR-017).
    Preconditions,
    /// `sequence_policy` — operator requirements (ADR-013 R9 / ADR-015).
    Requirements,
    /// `capacity_ledger` — the declared cap for the resource this step would consume
    /// (ADR-018 / ADR-010, #1455).
    Capacity,
    /// `effect_commitment` — the declared prior commitment of the same effect
    /// (ADR-019, #1455).
    Effects,
}

impl GateId {
    /// The stable string used in evidence and in refusals.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Preconditions => "preconditions",
            Self::Requirements => "requirements",
            Self::Capacity => "capacity",
            Self::Effects => "effects",
        }
    }
}

/// One finding, in evaluation order.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GateFinding {
    /// Which gate produced this.
    pub gate_id: GateId,
    /// The step it concerns.
    pub step: String,
    /// What that gate concluded.
    pub outcome: GateOutcome,
    /// The gate's own identifier for the rule/precondition, when it refused.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rule_id: Option<String>,
    /// Redacted detail — never parameter values, never stdout (SpanPrivacy).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    /// The gate's own finding payload, unread by any caller.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence: Option<serde_json::Value>,
}
