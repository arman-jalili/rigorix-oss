//! Capacity Ledger event payload schemas.
//!
//! @canonical .pi/architecture/modules/capacity-ledger.md#events
//! Implements: ISSUE-CONTRACT-FREEZE — LedgerEvent payload schemas
//! Issue: #1021
//!
//! Events describe state changes in the ledger: reservations, commits,
//! releases, and expiries. Consumers correlate them by `reservation_id`.
//!
//! # Contract (Frozen)
//! - Serialized as a JSON object with a `"type"` discriminator (snake_case)
//! - Each variant carries the full context needed by consumers
//! - No internal implementation details are exposed

use serde::{Deserialize, Serialize};

use super::{IdempotencyKey, ReservationId, ReservationState, ResourceId};

/// Events emitted by the Capacity Ledger module.
///
/// Serialized with a `"type"` tag, e.g.
/// `{"type":"reserved","resource":"gpu","amount":2,...}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LedgerEvent {
    /// A reservation was successfully created.
    Reserved {
        /// The claimed resource.
        resource: ResourceId,
        /// The new reservation identity.
        reservation_id: ReservationId,
        /// Units held.
        amount: u64,
        /// Retry identity of the reserve call.
        idempotency_key: IdempotencyKey,
    },

    /// A reservation was committed (capacity spent).
    Committed {
        /// The resource.
        resource: ResourceId,
        /// The reservation identity.
        reservation_id: ReservationId,
        /// Units consumed.
        amount: u64,
    },

    /// A reservation was released before consumption.
    Released {
        /// The resource.
        resource: ResourceId,
        /// The reservation identity.
        reservation_id: ReservationId,
        /// Units handed back.
        amount: u64,
        /// Why the hold was released.
        reason: String,
    },

    /// A reservation's lease lapsed before commit or release.
    Expired {
        /// The resource.
        resource: ResourceId,
        /// The reservation identity.
        reservation_id: ReservationId,
        /// Units reclaimed.
        amount: u64,
    },

    /// A reservation changed lifecycle state without an amount change.
    StateChanged {
        /// The resource.
        resource: ResourceId,
        /// The reservation identity.
        reservation_id: ReservationId,
        /// The state after the transition.
        state: ReservationState,
    },
}
