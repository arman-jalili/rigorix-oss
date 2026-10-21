//! NodeStateVocabulary — the cross-context state words.
//!
//! @canonical .pi/architecture/modules/effect-commitment.md#nodestatevocabulary
//! Implements: ISSUE-CONTRACT-FREEZE — `Dispatched` / `Ambiguous` vocabulary
//! Issue: #1113
//! ADR-019: without these words the context has nowhere to put its answer
//!
//! This is the **vocabulary contract** for the one edit this context requires in
//! `state_persistence`: `NodeStatus` gains `Dispatched` and `Ambiguous`, so a
//! node whose outcome is unknown no longer has to be recorded as `Completed` or
//! `Failed`.
//!
//! # Contract (Frozen)
//! - Wire labels are exactly `"dispatched"` and `"ambiguous"` (snake_case)
//! - `is_terminal` excludes both
//! - `Dispatched` and `Ambiguous` map from [`EffectState`] without loss
//! - `Committed` maps to the existing `"completed"` word; `NotCommitted` maps to
//!   the existing `"failed"` word

use crate::effect_commitment::domain::EffectState;

/// The vocabulary effect-commitment adds to `NodeStatus`.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct NodeStateVocabulary;

impl NodeStateVocabulary {
    /// Wire label for a node with a durable intent but an unknown outcome.
    pub const DISPATCHED: &'static str = "dispatched";

    /// Wire label for a node whose effect outcome is unknown.
    pub const AMBIGUOUS: &'static str = "ambiguous";

    /// Construct the vocabulary helper.
    pub const fn new() -> Self {
        Self
    }

    /// The wire label for an effect state.
    ///
    /// Terminal commitment reuses the existing node words so consumers that
    /// only understand the old vocabulary keep working.
    pub fn label(state: EffectState) -> &'static str {
        match state {
            EffectState::Dispatched => Self::DISPATCHED,
            EffectState::Ambiguous => Self::AMBIGUOUS,
            EffectState::Committed => "completed",
            EffectState::NotCommitted => "failed",
        }
    }

    /// The newly-added effect state for a wire label, if it is one of the words
    /// this context introduces.
    pub fn effect_state_for(label: &str) -> Option<EffectState> {
        match label {
            Self::DISPATCHED => Some(EffectState::Dispatched),
            Self::AMBIGUOUS => Some(EffectState::Ambiguous),
            _ => None,
        }
    }

    /// Whether a node label is terminal.
    ///
    /// Both new words are non-terminal: neither has answered the question.
    pub fn is_terminal(label: &str) -> bool {
        !matches!(label, Self::DISPATCHED | Self::AMBIGUOUS)
    }
}
