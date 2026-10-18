//! Reservation domain aggregate.
//!
//! @canonical .pi/architecture/modules/capacity-ledger.md#reservation
//! Implements: ISSUE-CONTRACT-FREEZE — Reservation + ReservationState + Lease
//! Issue: #1021
//!
//! The aggregate. A held claim on capacity with a lease and exactly one terminal
//! transition (committed, released, or expired).
//!
//! # Contract (Frozen)
//! - A reservation is created `Outstanding`
//! - `Committed`, `Released`, and `Expired` are terminal states
//! - A lapsed lease is **not proof of no effect**: if the outcome is unknown the
//!   reservation moves to `Ambiguous` and keeps its exposure until reconciled
//!   (late commit proves success; release proves no effect). `Ambiguous` is a
//!   reconciliation state, not a terminal one.
//! - The store clock decides expiry (I5), never the caller's clock

use std::fmt;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::{IdempotencyKey, ResourceId};

/// Identity of a single reservation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ReservationId(uuid::Uuid);

impl ReservationId {
    /// Generate a fresh, time-ordered reservation identity.
    pub fn new() -> Self {
        Self(uuid::Uuid::now_v7())
    }

    /// Wrap an existing UUID.
    pub fn from_uuid(id: uuid::Uuid) -> Self {
        Self(id)
    }

    /// The underlying UUID.
    pub fn as_uuid(&self) -> uuid::Uuid {
        self.0
    }
}

impl Default for ReservationId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ReservationId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A time-bounded hold on capacity.
///
/// `expires_at` is an absolute instant; the ledger's own clock is authoritative
/// when deciding expiry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lease {
    /// Instant at which the hold stops being a valid claim.
    pub expires_at: DateTime<Utc>,
}

impl Lease {
    /// Construct a lease that expires at `expires_at`.
    pub fn new(expires_at: DateTime<Utc>) -> Self {
        Self { expires_at }
    }

    /// Construct a lease expiring `ttl` from `now`.
    ///
    /// Preserves sub-second precision; a `ttl` too large for `chrono` saturates
    /// at the maximum representable duration rather than panicking.
    pub fn expiring_in(now: DateTime<Utc>, ttl: std::time::Duration) -> Self {
        let delta = chrono::Duration::from_std(ttl).unwrap_or(chrono::Duration::MAX);
        Self {
            expires_at: now + delta,
        }
    }

    /// Whether the lease has expired at `now`.
    pub fn is_expired_at(&self, now: DateTime<Utc>) -> bool {
        now >= self.expires_at
    }
}

/// Lifecycle state of a reservation.
///
/// Exactly one terminal transition is permitted; use [`ReservationState::is_terminal`]
/// to enforce it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ReservationState {
    /// Capacity is held but not yet spent.
    Outstanding,
    /// The lease lapsed and the outcome is unknown. Exposure is retained until
    /// the caller reconciles it with a late commit (proved success) or release
    /// (proved no effect). A lapsed lease is never proof that no effect happened.
    Ambiguous,
    /// Capacity was consumed (terminal).
    Committed,
    /// Capacity was handed back before consumption (terminal).
    Released,
    /// The lease lapsed and the reservation is definitively void (terminal).
    Expired,
}

impl ReservationState {
    /// Whether this state is terminal (no further transition is legal).
    ///
    /// `Ambiguous` is deliberately **not** terminal: it is the reconciliation
    /// state that a late commit or release resolves.
    pub fn is_terminal(&self) -> bool {
        !matches!(self, Self::Outstanding | Self::Ambiguous)
    }
}

/// Why a reservation state transition was rejected.
///
/// The aggregate permits exactly one terminal transition; every later attempt
/// is a programming error (or a retry after the state was already resolved).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ReservationTransitionError {
    /// The reservation already reached a terminal state.
    #[error("reservation {id} is already terminal ({state:?})")]
    AlreadyTerminal {
        /// The reservation that could not move.
        id: ReservationId,
        /// The terminal state it is stuck in.
        state: ReservationState,
    },

    /// An expiry transition was requested before the lease lapsed.
    #[error("reservation {id} lease has not expired yet")]
    LeaseNotExpired {
        /// The reservation whose lease is still live.
        id: ReservationId,
    },
}

/// A held claim on capacity.
///
/// # Fields (Frozen)
/// - `id: ReservationId` — stable identity
/// - `resource: ResourceId` — the claimed resource
/// - `amount: u64` — units held
/// - `state: ReservationState` — lifecycle state
/// - `lease: Lease` — absolute expiry
/// - `idempotency_key: IdempotencyKey` — retry identity of the reserve call
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reservation {
    /// Stable identity.
    pub id: ReservationId,

    /// The claimed resource.
    pub resource: ResourceId,

    /// Units held.
    pub amount: u64,

    /// Current lifecycle state.
    pub state: ReservationState,

    /// Absolute expiry of the hold.
    pub lease: Lease,

    /// Retry identity of the reserve call that created this reservation.
    pub idempotency_key: IdempotencyKey,
}

impl Reservation {
    /// Construct an `Outstanding` reservation.
    pub fn new(
        id: ReservationId,
        resource: ResourceId,
        amount: u64,
        lease: Lease,
        idempotency_key: IdempotencyKey,
    ) -> Self {
        Self {
            id,
            resource,
            amount,
            state: ReservationState::Outstanding,
            lease,
            idempotency_key,
        }
    }

    /// Whether this reservation has reached a terminal state.
    pub fn is_terminal(&self) -> bool {
        self.state.is_terminal()
    }

    /// Whether the lease has lapsed at `now`.
    pub fn is_expired_at(&self, now: DateTime<Utc>) -> bool {
        self.lease.is_expired_at(now)
    }

    /// Move to `to`, enforcing exactly one terminal transition.
    ///
    /// # Errors
    /// [`ReservationTransitionError::AlreadyTerminal`] if this reservation has
    /// already been committed, released, or expired.
    pub fn transition(mut self, to: ReservationState) -> Result<Self, ReservationTransitionError> {
        if self.state.is_terminal() {
            return Err(ReservationTransitionError::AlreadyTerminal {
                id: self.id,
                state: self.state,
            });
        }
        // `Ambiguous` may only move to a terminal state (reconciliation).
        if self.state == ReservationState::Ambiguous && !to.is_terminal() {
            return Err(ReservationTransitionError::AlreadyTerminal {
                id: self.id,
                state: self.state,
            });
        }
        self.state = to;
        Ok(self)
    }

    /// Commit the reservation (spend the held capacity).
    pub fn commit(self) -> Result<Self, ReservationTransitionError> {
        self.transition(ReservationState::Committed)
    }

    /// Release the reservation (hand the held capacity back).
    pub fn release(self) -> Result<Self, ReservationTransitionError> {
        self.transition(ReservationState::Released)
    }

    /// Mark the reservation `Ambiguous` once its lease has lapsed (I5): the
    /// outcome is unknown, so exposure must be retained until reconciliation.
    ///
    /// # Errors
    /// [`ReservationTransitionError::LeaseNotExpired`] before `now` reaches the
    /// lease deadline; [`ReservationTransitionError::AlreadyTerminal`] if it was
    /// already resolved.
    pub fn mark_ambiguous(self, now: DateTime<Utc>) -> Result<Self, ReservationTransitionError> {
        if !self.is_expired_at(now) {
            return Err(ReservationTransitionError::LeaseNotExpired { id: self.id });
        }
        self.transition(ReservationState::Ambiguous)
    }

    /// Expire the reservation definitively once its lease has lapsed (I5).
    ///
    /// Only use this when the caller has proof that no effect happened; a
    /// lapsed lease alone is unknown and must use [`Reservation::mark_ambiguous`].
    ///
    /// # Errors
    /// [`ReservationTransitionError::LeaseNotExpired`] before `now` reaches the
    /// lease deadline; [`ReservationTransitionError::AlreadyTerminal`] if it was
    /// already resolved.
    pub fn expire(self, now: DateTime<Utc>) -> Result<Self, ReservationTransitionError> {
        if !self.is_expired_at(now) {
            return Err(ReservationTransitionError::LeaseNotExpired { id: self.id });
        }
        self.transition(ReservationState::Expired)
    }
}
