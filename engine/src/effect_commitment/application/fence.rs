//! DependentFence and RetryFence — F1/F2 as ports.
//!
//! @canonical .pi/architecture/modules/effect-commitment.md#dependentfence
//! @canonical .pi/architecture/modules/effect-commitment.md#retryfence
//! Implements: ISSUE-CONTRACT-FREEZE — fence ports
//! Issue: #1113
//! ADR-019: F1 (dependents) and F2 (retry)
//!
//! `release_or_skip_dependents` must not release a dependent while any upstream
//! effect is `Ambiguous`, and the retry policy must not re-dispatch one. Either
//! failure lets the run proceed on top of an unknown — the exact double-spend
//! path this context exists to close.
//!
//! # Contract (Frozen)
//! - F1: a single `Ambiguous` upstream blocks release
//! - F2: an `Ambiguous` record blocks retry; a resolved record does not
//! - A new attempt after resolution is permitted

use crate::effect_commitment::domain::EffectRecord;

/// Decides whether dependents of a set of upstream effects may be released.
pub trait DependentFence: Send + Sync {
    /// `true` only when every upstream effect is safe to build on.
    fn may_release_dependents(&self, upstream: &[EffectRecord]) -> bool;
}

/// Decides whether a retry of a recorded effect may proceed.
pub trait RetryFence: Send + Sync {
    /// `true` only when the effect is not unsettled.
    fn may_retry(&self, record: &EffectRecord) -> bool;
}

/// The F1 rule as a pure predicate.
///
/// Dependents may be released only when **no** upstream effect is `Ambiguous`.
pub fn may_release_dependents(upstream: &[EffectRecord]) -> bool {
    !upstream.iter().any(EffectRecord::blocks_dependents)
}

/// The F2 rule as a pure predicate.
///
/// An effect may be retried only when it does not block retry.
pub fn may_retry(record: &EffectRecord) -> bool {
    !record.blocks_retry()
}
