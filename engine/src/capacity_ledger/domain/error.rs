//! LedgerError domain type.
//!
//! @canonical .pi/architecture/modules/capacity-ledger.md#ledgererror
//! Implements: ISSUE-CONTRACT-FREEZE — LedgerError typed failures
//! Issue: #1021
//!
//! Typed failures. `Insufficient` (no room) and `Unavailable` (control plane
//! unreachable) must remain distinguishable — an outage and a full cap are
//! different operational events, and the operator-facing surfaces depend on the
//! distinction.
//!
//! # Contract (Frozen)
//! - Exactly four variants: `Insufficient | Contended | Unavailable | LeaseExpired`
//! - `Unavailable` is the fail-closed bucket for every ambiguity (I4)
//! - No `anyhow` / stringly-typed errors cross this boundary

use thiserror::Error;

use super::{IdempotencyKey, ResourceId};

/// Typed failures for capacity-ledger operations.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum LedgerError {
    /// There is no room left for the requested amount.
    ///
    /// The resource is reachable and healthy; the cap is simply reached.
    #[error(
        "insufficient capacity for resource '{resource}': requested {requested}, available {available}"
    )]
    Insufficient {
        /// The contended resource.
        resource: ResourceId,
        /// Amount the caller asked for.
        requested: u64,
        /// Amount currently available.
        available: u64,
    },

    /// Two writers contended for the same capacity and the operation was lost.
    ///
    /// Safe to retry with the same idempotency key.
    #[error("resource '{resource}' is contended: {detail}")]
    Contended {
        /// The contended resource.
        resource: ResourceId,
        /// Human-readable detail for diagnostics.
        detail: String,
    },

    /// The ledger (or its backing store) could not be reached.
    ///
    /// Every ambiguous transport outcome maps here so the caller fails closed.
    #[error("capacity ledger is unavailable: {detail}")]
    Unavailable {
        /// Human-readable detail for diagnostics.
        detail: String,
    },

    /// The lease attached to a reservation has expired.
    #[error("lease expired for resource '{resource}' (idempotency key {key})")]
    LeaseExpired {
        /// The resource whose lease expired.
        resource: ResourceId,
        /// Key of the expired reservation.
        key: IdempotencyKey,
    },
}

impl LedgerError {
    /// Whether retrying with the same idempotency key may succeed.
    ///
    /// Contention and outages are transient; a full cap or an expired lease is
    /// not fixed by retrying the identical request.
    pub fn is_retriable(&self) -> bool {
        matches!(self, Self::Contended { .. } | Self::Unavailable { .. })
    }

    /// Whether the ledger itself could not be reached (an outage, not a cap).
    pub fn is_unavailable(&self) -> bool {
        matches!(self, Self::Unavailable { .. })
    }

    /// Whether the resource is healthy but has no room left.
    pub fn is_capacity_exhausted(&self) -> bool {
        matches!(self, Self::Insufficient { .. })
    }
}
