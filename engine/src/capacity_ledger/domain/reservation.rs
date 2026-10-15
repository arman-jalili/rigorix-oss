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
//! - The lease is authoritative: an expired reservation is no longer a claim
//! - The store clock decides expiry (I5), never the caller's clock

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

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

    /// Construct a lease expiring `ttl` seconds from `now`.
    pub fn expiring_in(now: DateTime<Utc>, ttl: std::time::Duration) -> Self {
        let secs = ttl.as_secs().min(i64::MAX as u64) as i64;
        Self {
            expires_at: now + chrono::Duration::seconds(secs),
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
    /// Capacity was consumed (terminal).
    Committed,
    /// Capacity was handed back before consumption (terminal).
    Released,
    /// The lease lapsed before commit or release (terminal).
    Expired,
}

impl ReservationState {
    /// Whether this state is terminal (no further transition is legal).
    pub fn is_terminal(&self) -> bool {
        !matches!(self, Self::Outstanding)
    }
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
}
