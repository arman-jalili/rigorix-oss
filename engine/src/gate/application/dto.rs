//! What a caller asks the gate, and what the gate answers.
//!
//! @canonical .pi/architecture/modules/gate.md
//! Issue: #1381 (Phase C.1)
//!
//! Three carriers, each named for the question it answers:
//!
//! | type | question |
//! |---|---|
//! | [`GateRequest`] | what is about to happen, under which pinned inputs, with which capabilities |
//! | [`GateDecision`] | may it happen, what was found in what order, and what is the digest a caller signs |
//! | [`GateEnvelopeBinding`] | the decision's identity, in the form a signer needs |
//!
//! Everything here is an application DTO. No domain type is introduced: a finding is
//! one of the services' findings, re-labelled with the gate that produced it.

use std::sync::Arc;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::audit::application::dto::PinnedInputs;
use crate::precondition::application::service::PreconditionService;
use crate::sequence_policy::application::service::SequencePolicyService;

/// One step about to be dispatched, as the gate needs to see it.
///
/// The gate reads a step exactly as the two services do — name, tool, parameters —
/// and nothing else. Approvals, retries and graph position are the caller's, and
/// keeping them out is what lets a browser supply a step with no graph behind it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GatedStep {
    /// Step name — the identity used across the engine and in evidence.
    pub name: String,
    /// The tool/action the step will invoke.
    pub tool: String,
    /// Full JSON parameter object of the step.
    pub parameters: serde_json::Value,
}

impl GatedStep {
    /// Build a step from its three parts.
    #[must_use]
    pub fn new(
        name: impl Into<String>,
        tool: impl Into<String>,
        parameters: serde_json::Value,
    ) -> Self {
        Self {
            name: name.into(),
            tool: tool.into(),
            parameters,
        }
    }
}

/// The capabilities the composition consults.
///
/// Both are `Option` because absence is a *decision the operator already made*, not
/// a missing wiring: no `.rigorix/preconditions.toml` means no dispatch-time gating
/// (fail-open-absent, ADR-017), and no sequence policy means no obligations. Modeling
/// absence as `None` keeps "there is no gate here" distinct from "a gate could not
/// answer", which is the distinction the refusal vocabulary rests on.
#[derive(Clone, Default)]
pub struct GatePorts {
    /// Dispatch-time precondition gate (ADR-017). `None` = not configured.
    pub precondition: Option<Arc<dyn PreconditionService>>,
    /// Operator requirements (ADR-013 R9 / ADR-015). `None` = no policy configured.
    pub sequence: Option<Arc<dyn SequencePolicyService>>,
}

impl std::fmt::Debug for GatePorts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GatePorts")
            .field("precondition", &self.precondition.is_some())
            .field("sequence", &self.sequence.is_some())
            .finish()
    }
}

impl GatePorts {
    /// No capability configured — the composition runs and passes.
    #[must_use]
    pub fn none() -> Self {
        Self::default()
    }

    /// Whether any capability is configured at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.precondition.is_none() && self.sequence.is_none()
    }
}

/// What a caller asks the gate.
///
/// # Why the pins are here and not in a port
///
/// ADR-014 §2 puts the decision instant and the execution identity in `PinnedInputs`
/// so two identical decisions produce identical bytes. The gate therefore *requires*
/// an execution id rather than defaulting one: a gate that minted its own id would be
/// non-deterministic by construction, and a gate that defaulted to the nil UUID would
/// make two different runs collide in the record.
#[derive(Debug)]
pub struct GateRequest<'a> {
    /// The step about to be dispatched.
    pub step: &'a GatedStep,
    /// The action being proposed, in the caller's words (for evidence only).
    pub proposed_action: &'a str,
    /// Whether the run carries an attested identity.
    ///
    /// A requirement may be *unmet* or *unpromotable* on this bit alone (ADR-015), so
    /// it is an input to the decision rather than a property the gate could infer.
    pub identity_attested: bool,
    /// The pinned instant and identity (ADR-014 §2). `execution_id` is required.
    pub pins: PinnedInputs,
    /// The capabilities the composition consults.
    pub ports: GatePorts,
}

/// Which gate produced a finding, by the module that owns it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GateId {
    /// `precondition` — the dispatch-time gate (ADR-017).
    Preconditions,
    /// `sequence_policy` — operator requirements (ADR-013 R9 / ADR-015).
    Requirements,
}

impl GateId {
    /// The stable string used in evidence and in refusals.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Preconditions => "preconditions",
            Self::Requirements => "requirements",
        }
    }
}

/// What one gate concluded about the step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GateOutcome {
    /// The gate ran and did not refuse.
    Passed,
    /// The gate ran and refused.
    Refused,
    /// The gate could not run. Always a refusal: an unavailable check must never
    /// read as a passed one (invariant 2).
    CouldNotEvaluate,
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

/// The verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GateVerdict {
    /// Every configured gate passed; the step may dispatch.
    Dispatch,
    /// A gate refused; the step must not dispatch.
    Refuse {
        /// Which gate refused.
        gate: GateId,
        /// The refusing rule's identifier, when it has one.
        rule_id: Option<String>,
        /// Redacted reason.
        reason: String,
    },
}

impl GateVerdict {
    /// Whether the step may dispatch.
    #[must_use]
    pub fn permits_dispatch(&self) -> bool {
        matches!(self, Self::Dispatch)
    }
}

/// The decision's identity, in the form a signer needs.
///
/// # Why a digest and not an envelope
///
/// `BuildEnvelopeInput` describes a *run*: template, planning prompt, token totals,
/// duration, git commit. A gate decision knows none of those and must not invent
/// them, so the gate publishes what it does own — the canonical bytes of the decision
/// and their digest — and the runtime that holds the run context wraps it. Same
/// division as the crate: the engine decides, the runtime signs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GateEnvelopeBinding {
    /// The execution this decision belongs to (from `PinnedInputs`).
    pub execution_id: Uuid,
    /// SHA-256 (hex) of the decision's canonical bytes.
    ///
    /// The bytes are the JSON encoding of `{verdict, findings}` in declaration order
    /// with no timestamps of their own, so two evaluations of the same input produce
    /// this same value — which is the determinism claim, as a value rather than a
    /// sentence.
    pub decision_digest: String,
    /// The instant the decision was evaluated with (ADR-014 §2), if pinned.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pinned_timestamp: Option<DateTime<Utc>>,
    /// Whether every input was pinned. A caller that signs an unpinned decision is
    /// signing a decision it cannot replay.
    pub pinned: bool,
}

/// What the gate answers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GateDecision {
    /// May the step dispatch.
    pub verdict: GateVerdict,
    /// Every finding, ordered exactly as the gates were evaluated.
    pub findings: Vec<GateFinding>,
    /// The identity a signer binds.
    pub envelope: GateEnvelopeBinding,
}

impl GateDecision {
    /// Whether the step may dispatch.
    #[must_use]
    pub fn permits_dispatch(&self) -> bool {
        self.verdict.permits_dispatch()
    }
}

/// Why the gate could not answer.
///
/// Every variant is a refusal at the caller (fail closed). Note what is *absent*: a
/// "no gate configured" variant. Absence is not an error — it is `GateVerdict::Dispatch`
/// with no findings, because the operator's decision not to gate is a decision.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum GateError {
    /// The request carried no execution id.
    ///
    /// Not defaulted: a gate that minted or defaulted one would make the digest
    /// non-reproducible or collide two runs in one record (ADR-014 §2).
    #[error("the gate request carries no execution id; pin one (ADR-014 §2)")]
    UnpinnedExecution,

    /// The precondition gate could not evaluate.
    #[error("the precondition gate could not evaluate: {detail}")]
    Precondition {
        /// Redacted detail.
        detail: String,
    },

    /// The requirements could not evaluate.
    #[error("the sequence-policy requirements could not evaluate: {detail}")]
    Requirements {
        /// Redacted detail.
        detail: String,
    },
}
