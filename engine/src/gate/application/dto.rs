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

/// The facts a gate cannot derive, and therefore cannot honestly invent.
///
/// # Why this type exists
///
/// A gate decision knows a verdict and its findings. Signed evidence knows a template,
/// a planning prompt, an author, token totals, a duration and a commit — none of which a
/// gate can produce. The first version left envelope assembly to the caller, which moved
/// the composition boundary instead of removing it: two runtimes that each assemble their
/// own envelope produce the same `decision_digest` and different signed bytes.
///
/// So the caller supplies the facts and the engine assembles the envelope. Every field is
/// required that a record requires: there is no `Default`, because a default here is a
/// fabrication that lands inside a signature.
#[derive(Debug, Clone)]
pub struct RunContext {
    /// Template that generated the run.
    pub template_id: String,
    /// The planning prompt (hashed into the envelope, never stored raw here).
    pub planning_prompt: String,
    /// What triggered the run (e.g. `rigorix_cli`).
    pub source: Option<String>,
    /// Repository the run acted on (`owner/name`).
    pub repository: Option<String>,
    /// Who triggered it.
    pub author: Option<String>,
    /// Attributed identity, when the run carries one.
    pub identity: Option<crate::identity::domain::IdentityRef>,
    /// Tokens the run consumed.
    pub total_tokens: u32,
    /// Wall-clock duration, in milliseconds.
    pub duration_ms: u64,
    /// Commit the run observed.
    pub git_commit: Option<String>,
    /// Branch the run observed.
    pub git_branch: Option<String>,
    /// The key source's answer, when this runtime signs.
    ///
    /// `Secret` rather than `String` so a `Debug` of a request cannot print a key. `None`
    /// produces an unsigned envelope — a record that says what happened without claiming
    /// who attested it, which is a different thing from a forged signature.
    pub signing_key: Option<crate::configuration::domain::Secret>,
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
    /// The run's facts, when the caller wants a signed envelope rather than a digest.
    ///
    /// `None` keeps the digest-only decision a library caller needs: a decision is still
    /// a decision without a record of who ran it.
    pub run_context: Option<RunContext>,
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
    /// The gate ran and requires a human before the step may proceed.
    ///
    /// Distinct from both neighbours on purpose: `Passed` would let the step dispatch,
    /// and `Refused` would say the decision is final. A promotion is a decision that is
    /// *not yet made* — and #1392 exists because this third value had no home in the type,
    /// so `matches!(action, Deny)` folded it into `Passed`.
    Promoted,
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
    /// A gate promoted the step: it may not dispatch until a human approves it.
    ///
    /// The gate reports; the dispatcher acts. Setting `requires_approval` on the plan from
    /// here would write a policy read into the plan, and a plan reused under a changed
    /// policy would carry a stale flag — so the verdict is consulted per dispatch attempt
    /// instead, which is also what makes it correct across a resume.
    Promote {
        /// Which gate promoted it.
        gate: GateId,
        /// The promoting rule's identifier.
        rule_id: String,
        /// Redacted reason.
        reason: String,
    },
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
    /// Whether the step may dispatch now.
    ///
    /// A promotion is *not* dispatch: the step is waiting for a human, and a caller that
    /// treats "not refused" as "may proceed" reintroduces the defect this variant removed.
    #[must_use]
    pub fn permits_dispatch(&self) -> bool {
        matches!(self, Self::Dispatch)
    }

    /// Whether the step is waiting for a human.
    #[must_use]
    pub fn awaits_approval(&self) -> bool {
        matches!(self, Self::Promote { .. })
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

/// The signed record of a decision, when the caller supplied a [`RunContext`].
///
/// Equality is by the bytes and the hash, not by the envelope's own derives: two records
/// are the same record when a verifier would see the same bytes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateEnvelopeEvidence {
    /// The envelope the engine built and signed.
    pub envelope: crate::audit::domain::AuditEnvelope,
    /// Its canonical bytes — the exact bytes a signature covers (ADR-010).
    pub canonical_bytes: Vec<u8>,
    /// Its hash, as a verifier computes it.
    pub envelope_hash: String,
}

/// What the gate answers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateDecision {
    /// May the step dispatch.
    pub verdict: GateVerdict,
    /// Every finding, ordered exactly as the gates were evaluated.
    pub findings: Vec<GateFinding>,
    /// The identity a signer binds.
    pub envelope: GateEnvelopeBinding,
    /// The signed record, when a [`RunContext`] was supplied.
    ///
    /// Assembled here rather than by each runtime, so the CLI and the crate cannot
    /// produce matching digests over differing signed bytes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub envelope_evidence: Option<GateEnvelopeEvidence>,
}

impl PartialEq for GateDecision {
    /// Two decisions are the same decision when the verdict, the findings and — if a record
    /// was built — the canonical bytes and hash agree. Comparing the envelope's fields one
    /// by one would make equality a property of the factory's internals rather than of what
    /// a verifier sees.
    fn eq(&self, other: &Self) -> bool {
        let evidence_eq = match (&self.envelope_evidence, &other.envelope_evidence) {
            (None, None) => true,
            (Some(a), Some(b)) => {
                a.canonical_bytes == b.canonical_bytes && a.envelope_hash == b.envelope_hash
            }
            _ => false,
        };
        self.verdict == other.verdict
            && self.findings == other.findings
            && self.envelope == other.envelope
            && evidence_eq
    }
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

    /// The composition was not ready on its first poll.
    ///
    /// The slice's gate path must resolve without awaiting I/O: a port that awaits real
    /// I/O here is a build defect the caller is told about, not a hang or a panic
    /// (ADR-012 forbids the panic). See `evaluate_gate_sync`.
    #[error(
        "the gate composition was not ready on its first poll — a port on this path awaits I/O"
    )]
    NotReady,

    /// The envelope could not be assembled or signed.
    #[error("the gate decision could not be recorded: {detail}")]
    Envelope {
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
