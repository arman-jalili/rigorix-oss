//! EffectState — the four-state commitment machine.
//!
//! @canonical .pi/architecture/modules/effect-commitment.md#effectstate
//! Implements: ISSUE-CONTRACT-FREEZE — EffectState lifecycle + fencing predicates
//! Issue: #1113
//! ADR-019: normative state machine
//!
//! ```text
//! Intent --dispatch-----------------------> Dispatched
//! Dispatched --observed success-----------> Committed      (terminal)
//! Dispatched --observed definite failure--> NotCommitted   (terminal)
//! Dispatched --lost response / deadline / crash --> Ambiguous   (NOT terminal)
//! Ambiguous --resolver + provider evidence--> Committed | NotCommitted
//! ```
//!
//! # Contract (Frozen)
//! - `Committed` and `NotCommitted` are terminal
//! - `Dispatched` and `Ambiguous` are **not** terminal
//! - `Ambiguous` blocks dependents (F1) and retry (F2)
//! - `Ambiguous -> Dispatched` is illegal; a retry while unsettled is refused

use serde::{Deserialize, Serialize};

/// Lifecycle state of a recorded effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectState {
    /// The intent is durable and the effect has been (or is being) sent.
    Dispatched,

    /// Provider evidence proves the effect committed (terminal).
    Committed,

    /// Provider evidence proves the effect did not commit (terminal).
    NotCommitted,

    /// The response was lost, the deadline passed, or the process died while
    /// the effect was in flight. The outcome is unknown until a resolver
    /// presents provider evidence. Deliberately **not** terminal.
    Ambiguous,
}

impl EffectState {
    /// Whether no further transition is legal.
    ///
    /// `Dispatched` still has to settle; `Ambiguous` still has to be resolved.
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Committed | Self::NotCommitted)
    }

    /// Whether this state blocks dependents from being released (F1).
    pub fn blocks_dependents(&self) -> bool {
        matches!(self, Self::Ambiguous)
    }

    /// Whether this state blocks a retry (F2).
    pub fn blocks_retry(&self) -> bool {
        matches!(self, Self::Ambiguous)
    }

    /// Whether the outcome is currently unknown.
    pub fn is_ambiguous(&self) -> bool {
        matches!(self, Self::Ambiguous)
    }

    /// The canonical snake_case name of this state.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Dispatched => "dispatched",
            Self::Committed => "committed",
            Self::NotCommitted => "not_committed",
            Self::Ambiguous => "ambiguous",
        }
    }
}

impl std::fmt::Display for EffectState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}
