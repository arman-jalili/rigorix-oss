//! ReservationGuard RAII contract.
//!
//! @canonical .pi/architecture/modules/capacity-ledger.md#reservationguard
//! Implements: ISSUE-CONTRACT-FREEZE — ReservationGuard RAII guard
//! Issue: #1021
//! ADR-008: RAII reservation pattern
//!
//! RAII binding of a grant to a step's lifetime: commit on success, **release on
//! drop**. Release-on-drop is not optional — if release happens only on an
//! explicit error branch, a panic or early return leaks capacity until the lease
//! expires.
//!
//! # Contract (Frozen)
//! - Drop without [`ReservationGuard::commit`] MUST release the hold
//! - `commit` consumes the guard (by `Box<Self>`) and is terminal
//! - The guard is `Send + Sync + Debug` so it can travel inside DTOs

use async_trait::async_trait;

use crate::capacity_ledger::domain::{Grant, LedgerError, Reservation, ReservationId};

/// RAII handle owning a single reservation.
///
/// Created by `CapacityLedgerService::reserve`. Implementations MUST release the
/// reservation when dropped without a successful `commit`.
#[async_trait]
pub trait ReservationGuard: Send + Sync + std::fmt::Debug {
    /// Identity of the reservation held by this guard.
    fn reservation_id(&self) -> ReservationId;

    /// The underlying grant, or `None` once consumed by `commit`.
    fn grant(&self) -> Option<&Grant>;

    /// Whether [`ReservationGuard::commit`] has completed.
    fn is_committed(&self) -> bool;

    /// Whether the hold has been released (explicitly or on drop).
    fn is_released(&self) -> bool;

    /// Commit the underlying reservation with the reserved amount.
    ///
    /// Consumes the guard. After a successful commit, dropping the guard no
    /// longer releases anything.
    async fn commit(self: Box<Self>) -> Result<Reservation, LedgerError>;
}
