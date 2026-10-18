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
//! - Drop without [`ReservationGuard::commit`] **attempts** a best-effort release.
//!   Drop cannot await, so it is not a correctness guarantee: use an explicit
//!   commit/release for correctness, with the lease as the durable backstop.
//! - `commit` consumes the guard (by `Box<Self>`) and is terminal
//! - The guard is `Send + Sync + Debug` so it can travel inside DTOs
//! - The guard deliberately does **not** hand back its [`Grant`](crate::capacity_ledger::domain::Grant):
//!   a caller who could obtain the grant could commit it directly and bypass
//!   release-on-drop. Callers observe `reservation_id` and the state flags only.

use async_trait::async_trait;

use crate::capacity_ledger::domain::{LedgerError, Reservation, ReservationId};

/// RAII handle owning a single reservation.
///
/// Created by `CapacityLedgerService::reserve`. Implementations MUST release the
/// reservation when dropped without a successful `commit`.
#[async_trait]
pub trait ReservationGuard: Send + Sync + std::fmt::Debug {
    /// Identity of the reservation held by this guard.
    fn reservation_id(&self) -> ReservationId;

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
