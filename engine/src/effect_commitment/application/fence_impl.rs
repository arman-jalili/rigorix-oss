//! Default fence implementations.
//!
//! @canonical .pi/architecture/modules/effect-commitment.md#dependentfence
//! @canonical .pi/architecture/modules/effect-commitment.md#retryfence
//! Implements: ISSUE-EFFECT-COMMITMENT-18 — concrete retry fence
//! Issue: #1131
//! ADR-019: F1 (dependents) and F2 (retry)
//!
//! The pure predicates live in `fence.rs`; these zero-sized types make them
//! injectable as the frozen ports. The dispatch seam consults the retry fence
//! before sending an effect whose write-ahead read came back unsettled.

use crate::effect_commitment::domain::EffectRecord;

use super::fence::{DependentFence, RetryFence, may_release_dependents, may_retry};

/// The default F1 fence: no release while any upstream is ambiguous.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct DefaultDependentFence;

impl DependentFence for DefaultDependentFence {
    fn may_release_dependents(&self, upstream: &[EffectRecord]) -> bool {
        may_release_dependents(upstream)
    }
}

/// The default F2 fence: no retry while the effect is ambiguous.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct DefaultRetryFence;

impl RetryFence for DefaultRetryFence {
    fn may_retry(&self, record: &EffectRecord) -> bool {
        may_retry(record)
    }
}
