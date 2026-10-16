//! Grant domain type.
//!
//! @canonical .pi/architecture/modules/capacity-ledger.md#grant
//! Implements: ISSUE-CONTRACT-FREEZE — Grant opaque reservation handle
//! Issue: #1021
//!
//! The opaque handle returned by `reserve`. It binds the resource, the amount,
//! the lease, and the idempotency key of the reservation that produced it.
//!
//! # Contract (Frozen)
//! - **Must not be constructible by a caller.** All fields are private and there
//!   is deliberately no public constructor. A forged grant would be a way to skip
//!   reservation, which this type exists to prevent.
//! - **Not `Deserialize`.** A grant cannot be reconstructed from JSON.
//! - Only a `CapacityLedger` adapter may issue a grant (later via a
//!   crate-internal constructor).

use super::{IdempotencyKey, Lease, ReservationId, ResourceId};

/// Opaque proof-of-reservation handle returned by `CapacityLedger::reserve`.
#[derive(Debug, Clone)]
pub struct Grant {
    reservation_id: ReservationId,
    resource: ResourceId,
    amount: u64,
    lease: Lease,
    idempotency_key: IdempotencyKey,
}

impl Grant {
    /// Issue a grant after a successful reservation.
    ///
    /// Deliberately `pub(crate)`: only a [`CapacityLedger`] adapter may mint a
    /// grant, and only after it has actually reserved capacity. A public
    /// constructor (or a `From<Reservation>` impl) would let a caller forge a
    /// grant and bypass reservation entirely.
    ///
    /// [`CapacityLedger`]: crate::capacity_ledger::interfaces::CapacityLedger
    #[allow(dead_code)] // Consumed by InProcessLedger/HttpLedger in later issues.
    pub(crate) fn issue(
        reservation_id: ReservationId,
        resource: ResourceId,
        amount: u64,
        lease: Lease,
        idempotency_key: IdempotencyKey,
    ) -> Self {
        Self {
            reservation_id,
            resource,
            amount,
            lease,
            idempotency_key,
        }
    }

    /// Consume the grant into its constituent parts (for adapter commit/release).
    #[allow(dead_code)] // Consumed by InProcessLedger/HttpLedger in later issues.
    pub(crate) fn into_parts(self) -> (ReservationId, ResourceId, u64, Lease, IdempotencyKey) {
        (
            self.reservation_id,
            self.resource,
            self.amount,
            self.lease,
            self.idempotency_key,
        )
    }

    /// The reservation this grant proves.
    pub fn reservation_id(&self) -> ReservationId {
        self.reservation_id
    }

    /// The claimed resource.
    pub fn resource(&self) -> &ResourceId {
        &self.resource
    }

    /// The amount held.
    pub fn amount(&self) -> u64 {
        self.amount
    }

    /// The lease attached to the hold.
    pub fn lease(&self) -> &Lease {
        &self.lease
    }

    /// The retry identity of the reserve call that produced this grant.
    pub fn idempotency_key(&self) -> &IdempotencyKey {
        &self.idempotency_key
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    use chrono::Utc;

    fn grant() -> Grant {
        Grant::issue(
            ReservationId::new(),
            ResourceId::new("gpu"),
            2,
            Lease::expiring_in(Utc::now(), Duration::from_secs(60)),
            IdempotencyKey::new("k1"),
        )
    }

    #[test]
    fn issued_grant_exposes_the_reservation_it_proves() {
        let grant = grant();
        assert_eq!(grant.resource(), &ResourceId::new("gpu"));
        assert_eq!(grant.amount(), 2);
        assert_eq!(grant.idempotency_key(), &IdempotencyKey::new("k1"));
        assert!(!grant.lease().is_expired_at(Utc::now()));
        assert!(grant.reservation_id().as_uuid().get_version_num() >= 7);
    }

    #[test]
    fn into_parts_round_trips_the_binding() {
        let grant = grant();
        let id = grant.reservation_id();
        let (reservation_id, resource, amount, lease, key) = grant.into_parts();
        assert_eq!(reservation_id, id);
        assert_eq!(resource, ResourceId::new("gpu"));
        assert_eq!(amount, 2);
        assert_eq!(key, IdempotencyKey::new("k1"));
        assert!(!lease.is_expired_at(Utc::now()));
    }
}
