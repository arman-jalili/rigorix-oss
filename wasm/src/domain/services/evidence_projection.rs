//! `EvidenceProjection` — engine findings to ordered `StepOutcome`s.
//!
//! @canonical .pi/architecture/modules/portable-session.md#domain-services--application-services--infrastructure-services
//! Implements: Contract Freeze — EvidenceProjection
//! Issue: #1294 (contract freeze)
//!
//! The engine reports findings; the record needs steps. This projection is the only
//! place that translation happens, so there is exactly one ordering rule and exactly one
//! place where `NotReached` can be lost — and it is tested (criterion 9).
//!
//! # Contract (Frozen)
//! - Output order **is** the input order; no sorting, no grouping, no de-duplication
//! - `NotReached` survives as `NotReached`; it is never collapsed into `Refused`
//! - The projection never invents a step, and never drops one
//! - Wording comes from [`RefusalVocabulary`], never from the engine's `Display`

use crate::domain::engine_seam::EngineFinding;
use crate::domain::outcome::StepOutcome;

/// Projects engine findings into the run's ordered steps.
pub trait EvidenceProjection {
    /// Project findings, preserving engine order and `NotReached`.
    fn project(&self, findings: &[EngineFinding]) -> Vec<StepOutcome>;
}

/// A boxed projection is still a projection: callers that choose the policy at run time
/// (the conformance runner) should not need a second trait.
impl EvidenceProjection for Box<dyn EvidenceProjection> {
    fn project(&self, findings: &[EngineFinding]) -> Vec<StepOutcome> {
        (**self).project(findings)
    }
}

/// A borrowed projection, for services that hold a reference rather than a value.
impl<T: EvidenceProjection + ?Sized> EvidenceProjection for &T {
    fn project(&self, findings: &[EngineFinding]) -> Vec<StepOutcome> {
        (**self).project(findings)
    }
}
