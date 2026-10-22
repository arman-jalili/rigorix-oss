//! EffectReconciler — the startup sweep for orphaned `Dispatched` records.
//!
//! @canonical .pi/architecture/modules/effect-commitment.md#effectcommitmentservice
//! Implements: ISSUE-1162 — expired `Dispatched` becomes `Ambiguous`
//! Issue: #1164
//! ADR-019: the outcome is unknown, so the record must say so
//!
//! A process can die after `begin_effect` wrote the record but before the effect
//! settles. The record stays `Dispatched`; `may_retry` correctly blocks only
//! `Ambiguous`, so nothing separates "in flight now" from "in flight when the
//! owner died". A resumed run would retry an effect that may already have
//! happened.
//!
//! This sweep closes that window at startup: a `Dispatched` record older than
//! the dispatch lease has no live owner, so it is moved to `Ambiguous` — the
//! only honest state. The transition is conditional (the adapter re-checks the
//! state), so N composition roots sweeping concurrently converge on one result.
//!
//! # Contract (Frozen)
//! - Only `Dispatched` records can be swept; terminal records are settled
//! - The sweep never resolves: it may not claim committed or not-committed
//! - A sweep failure is fail-closed; the caller refuses to start
//! - A record that is still fresh is untouched (it may be in flight elsewhere)

use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;

use crate::effect_commitment::domain::{CommitmentError, EffectQuery};
use crate::effect_commitment::interfaces::EffectCommitmentLedger;

/// How long a `Dispatched` record may exist without a live owner.
///
/// Deliberately longer than the capacity reservation lease: a node may
/// legitimately run for minutes, and a sweep must never move a live dispatch to
/// `Ambiguous`. The value is the maximum crash window an operator has to accept.
pub const DEFAULT_DISPATCH_LEASE: Duration = Duration::from_secs(15 * 60);

/// Sweeps expired `Dispatched` records to `Ambiguous` at startup.
pub struct EffectReconciler {
    ledger: Arc<dyn EffectCommitmentLedger>,
    lease: Duration,
}

impl EffectReconciler {
    /// Construct a reconciler with an explicit lease.
    pub fn new(ledger: Arc<dyn EffectCommitmentLedger>, lease: Duration) -> Self {
        Self { ledger, lease }
    }

    /// Construct a reconciler with [`DEFAULT_DISPATCH_LEASE`].
    pub fn with_default_lease(ledger: Arc<dyn EffectCommitmentLedger>) -> Self {
        Self::new(ledger, DEFAULT_DISPATCH_LEASE)
    }

    /// Whether this reconciler's owner may sweep at all.
    ///
    /// False for a shared ledger: the service owns the sweep, and a client that
    /// swept would compare another process's timestamps against its own clock.
    pub fn can_sweep(&self) -> bool {
        self.ledger.can_sweep_dispatched()
    }

    /// The configured lease.
    pub fn lease(&self) -> Duration {
        self.lease
    }

    /// Move every expired `Dispatched` record to `Ambiguous`.
    ///
    /// Returns the number of records this caller swept. A concurrent root that
    /// won the conditional transition is benign: the record is already
    /// `Ambiguous`, which is the state this sweep exists to produce.
    ///
    /// # Errors
    /// A transport/storage failure is returned unchanged; the caller must fail
    /// closed rather than start with an unknown backlog.
    pub async fn reconcile(&self) -> Result<usize, CommitmentError> {
        if !self.ledger.can_sweep_dispatched() {
            tracing::debug!(
                "effect-commitment: ledger is shared; the service owns the Dispatched sweep, \
                 this root does not sweep it (#1173)"
            );
            return Ok(0);
        }
        let now = Utc::now();
        let candidates = self.ledger.query(EffectQuery::dispatched(None)).await?;
        let mut swept = 0usize;

        for record in candidates {
            if !record.is_dispatch_expired(now, self.lease) {
                continue;
            }
            let reason = format!(
                "deadline exceeded: dispatched for longer than {}s with no live owner",
                self.lease.as_secs()
            );
            match self.ledger.mark_ambiguous(record.id(), reason).await {
                Ok(ambiguous) => {
                    swept += 1;
                    tracing::warn!(
                        record = %ambiguous.id(),
                        dag = %ambiguous.intent().dag_id,
                        node = %ambiguous.intent().node_id,
                        lease_secs = self.lease.as_secs(),
                        "effect was Dispatched past its lease; marked Ambiguous at startup"
                    );
                }
                // Another composition root swept it first, or the effect settled
                // while we were looking. Both are success for this sweep.
                Err(CommitmentError::Contended { .. } | CommitmentError::AlreadySettled { .. }) => {
                }
                Err(error) => return Err(error),
            }
        }

        Ok(swept)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_lease_is_longer_than_a_capacity_lease() {
        // The sweep must never race a live dispatch. Capacity's default
        // reservation lease is 60s; this is deliberately more generous.
        assert!(DEFAULT_DISPATCH_LEASE >= Duration::from_secs(5 * 60));
    }
}
