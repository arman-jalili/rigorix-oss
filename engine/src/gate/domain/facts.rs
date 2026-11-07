//! The facts a gate consults at the moment of consequence, and the arithmetic it owns.
//!
//! @canonical .pi/architecture/modules/gate.md
//! Issue: #1455 (CONSEQ-1, epic #1443)
//!
//! # Facts in, decision out
//!
//! Two of the four capabilities the engine consults are composed in the dispatch path, which
//! `attempt_run` never reaches. The seam is not a second implementation of either; it is a way
//! for a host to *declare what is true* and for the engine to decide.
//!
//! That distinction is the whole point of this module. A trait that answered `allowed`/`refused`
//! would put the decision in the adapter, and then two runtimes could disagree about the same
//! declared cap — which is exactly the class of defect ADR-013 exists to prevent. So
//! [`CapacityFacts`] returns an [`Exposure`] and an amount, and [`Exposure::would_fit`] — the
//! ledger's own function, the same one the transactional path calls — makes the call.
//!
//! # Absence is free
//!
//! Both ports are `Option` on [`GatePorts`](crate::gate::application::request::GatePorts): `None`
//! is *"no obligation was declared"*, not missing wiring (the same language the precondition and
//! sequence ports already use). A composition with no capacity facts declares the exposure
//! [`None`] and produces no finding, which is why the three conformance vectors recorded before
//! this module existed still produce the same decision bytes.
//!
//! # What is not here
//!
//! No storage, no clock, no provider. The instant of a committed effect is *reported* by the
//! caller, not read here: a gate that read the clock would make two identical decisions differ.

use chrono::{DateTime, Utc};

use crate::capacity_ledger::domain::Exposure;

/// What the caller knows about the resource this attempt would consume.
///
/// Implemented by for an adapter, not a calculator: the two answers are the ledger's own
/// shape and the amount being asked for. There is deliberately no `fn allowed(&self) -> bool` —
/// see the module docs.
pub trait CapacityFacts: Send + Sync {
    /// The resource's exposure as last recorded, or `None` when no obligation is declared.
    fn exposure(&self) -> Option<Exposure>;

    /// The amount this attempt would consume.
    fn requested(&self) -> u64;
}

/// A record of the same effect having already committed.
///
/// Identity is the tuple the reconciliation path already uses to decide whether two effects
/// are the same one: who performed it, when, and the provider's record identity. It is created
/// here rather than borrowed, because `effect_commitment`'s domain describes the *outcome* of a
/// commitment and had no type for a prior commitment's identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommittedEffect {
    /// The provider's record identity (a receipt, an invoice number, a ledger entry).
    pub record_id: String,
    /// Who performed it.
    pub actor: String,
    /// When it committed.
    pub instant: DateTime<Utc>,
}

impl CommittedEffect {
    /// Construct a committed effect's identity.
    pub fn new(
        record_id: impl Into<String>,
        actor: impl Into<String>,
        instant: DateTime<Utc>,
    ) -> Self {
        Self {
            record_id: record_id.into(),
            actor: actor.into(),
            instant,
        }
    }

    /// The identity as the refusal names it: the record and the instant.
    ///
    /// The instant is rendered as RFC 3339 in UTC, which is what the record itself carries.
    /// Formatting lives here so two runtimes cannot word the same duplicate differently.
    #[must_use]
    pub fn describe(&self) -> String {
        format!(
            "{} at {} by {}",
            self.record_id,
            self.instant
                .to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
            self.actor
        )
    }
}

/// What the caller knows about the effect this attempt would commit.
pub trait EffectFacts: Send + Sync {
    /// The prior commitment of the *same* effect, when the caller has one to declare.
    ///
    /// `None` means "this effect has not committed", which is a stronger claim than "I do not
    /// know" — an adapter that cannot tell must not return `None`, it must fail to supply the
    /// port at all, so that "no obligation" and "no answer" stay distinguishable.
    fn committed(&self) -> Option<CommittedEffect>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_committed_effect_describes_its_record_and_instant_without_subsecond_noise() {
        let effect = CommittedEffect::new(
            "QR-2041",
            "finance",
            DateTime::parse_from_rfc3339("2026-11-04T14:02:00Z")
                .expect("fixture instant")
                .with_timezone(&Utc),
        );
        assert_eq!(
            effect.describe(),
            "QR-2041 at 2026-11-04T14:02:00Z by finance"
        );
    }
}
