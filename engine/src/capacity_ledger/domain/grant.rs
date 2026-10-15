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
