//! Repository interface for the Capacity Ledger.
//!
//! @canonical .pi/architecture/modules/capacity-ledger.md#overview
//! Implements: ISSUE-CONTRACT-FREEZE — repository/DAO interface
//! Issue: #1021
//!
//! Storage is an adapter (ADR-018). This trait is the seam between the ledger's
//! domain model and whatever store backs it. Column names, table layouts, and
//! connection handles are deliberately absent: they are out of bounds for the
//! frozen contract.
//!
//! # Contract (Frozen)
//! - All methods are async and return [`LedgerError`]
//! - Idempotency lookup is by key, scoped per resource by the caller
//! - No store type appears in any signature

use async_trait::async_trait;

use crate::capacity_ledger::domain::{
    Exposure, IdempotencyKey, LedgerError, Reservation, ReservationId, ResourceId, ResourceSpec,
};

/// Persistence seam for the capacity ledger.
///
/// Implementations may be in-memory (single execution) or a real contested
/// store reached over a network. The port guarantees (I1–I6) are the
/// implementation's responsibility, not this trait's.
#[async_trait]
pub trait CapacityLedgerRepository: Send + Sync {
    /// Store or replace a resource spec.
    async fn put_spec(&self, spec: &ResourceSpec) -> Result<(), LedgerError>;

    /// Load a resource spec.
    async fn get_spec(&self, resource: &ResourceId) -> Result<Option<ResourceSpec>, LedgerError>;

    /// Insert or update a reservation.
    async fn put_reservation(&self, reservation: &Reservation) -> Result<(), LedgerError>;

    /// Load a reservation by identity.
    async fn get_reservation(&self, id: &ReservationId)
    -> Result<Option<Reservation>, LedgerError>;

    /// Find the reservation previously created with `key` (I2).
    async fn find_by_idempotency_key(
        &self,
        resource: &ResourceId,
        key: &IdempotencyKey,
    ) -> Result<Option<Reservation>, LedgerError>;

    /// List reservations for a resource (used to compute exposure).
    async fn list_reservations(
        &self,
        resource: &ResourceId,
    ) -> Result<Vec<Reservation>, LedgerError>;

    /// Compute the aggregate exposure for a resource.
    async fn exposure(&self, resource: &ResourceId) -> Result<Exposure, LedgerError>;
}
