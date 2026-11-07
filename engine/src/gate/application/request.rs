//! What a caller asks the gate, and the ports it hands over.
//!
//! @canonical .pi/architecture/modules/gate.md

use std::sync::Arc;

use crate::audit::application::dto::PinnedInputs;
use crate::gate::domain::{CapacityFacts, EffectFacts, GatedStep};
use crate::precondition::application::service::PreconditionService;
use crate::sequence_policy::application::service::SequencePolicyService;

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
    /// The resource exposure and amount for the step's declared cap (ADR-018). `None` = no
    /// capacity obligation declared for this step.
    ///
    /// A *fact* port, not a predicate: the ledger's own `Exposure::would_fit` decides, so two
    /// runtimes cannot disagree about the same declared cap (#1455).
    pub capacity: Option<Arc<dyn CapacityFacts>>,
    /// The prior commitment of the same effect (ADR-019). `None` = no effect obligation.
    pub effect: Option<Arc<dyn EffectFacts>>,
}

impl std::fmt::Debug for GatePorts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GatePorts")
            .field("precondition", &self.precondition.is_some())
            .field("sequence", &self.sequence.is_some())
            .field("capacity", &self.capacity.is_some())
            .field("effect", &self.effect.is_some())
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
        self.precondition.is_none()
            && self.sequence.is_none()
            && self.capacity.is_none()
            && self.effect.is_none()
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
