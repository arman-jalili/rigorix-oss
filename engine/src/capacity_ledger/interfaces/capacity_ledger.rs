//! The `CapacityLedger` port.
//!
//! @canonical .pi/architecture/modules/capacity-ledger.md#capacityledger
//! Implements: ISSUE-CONTRACT-FREEZE — CapacityLedger port with invariants I1–I6
//! Issue: #1021
//! ADR-018: distributed capacity ledger
//!
//! The port. The invariants (I1–I6) below are the specification; the signatures
//! are incidental. One contract suite runs against every adapter.
//!
//! # Invariants (Frozen)
//!
//! - **I1 — Atomic / linearizable** per resource+window: concurrent reserves
//!   never observe a partially-applied reservation.
//! - **I2 — Idempotent**: a retried `reserve` with the same `IdempotencyKey`
//!   yields the same reservation and never over-reserves.
//! - **I3 — Leased**: every granted hold carries a lease and is reclaimed when
//!   the lease lapses.
//! - **I4 — Fail-closed**: any ambiguity (transport error, malformed response,
//!   timeout) maps to [`LedgerError::Unavailable`]; the caller never assumes
//!   capacity it did not receive.
//! - **I5 — Store clock**: expiry is decided by the ledger's clock, never the
//!   caller's.
//! - **I6 — No claim inflation**: `committed + outstanding` never exceeds
//!   `hard_limit`.

use async_trait::async_trait;

use crate::capacity_ledger::domain::{
    Exposure, Grant, IdempotencyKey, LedgerError, Reservation, ResourceId, ResourceSpec,
};

/// The capacity-ledger port (ADR-018).
///
/// Program against this trait; adapters (`InProcessLedger`, `HttpLedger`)
/// implement it. No storage type appears in this contract.
#[async_trait]
pub trait CapacityLedger: Send + Sync {
    /// Declare (or replace) the spec for a resource.
    ///
    /// Idempotent by `ResourceSpec::id`. Replacing a spec with a lower
    /// `hard_limit` than current exposure MUST be rejected as
    /// [`LedgerError::Insufficient`].
    async fn declare_resource(&self, spec: ResourceSpec) -> Result<(), LedgerError>;

    /// Reserve `amount` units of `resource`, bound to `key`.
    ///
    /// Returns an opaque [`Grant`]. Repeating the call with the same `key`
    /// returns an equivalent grant without reserving twice (I2).
    async fn reserve(
        &self,
        resource: &ResourceId,
        amount: u64,
        key: IdempotencyKey,
    ) -> Result<Grant, LedgerError>;

    /// Commit a previously issued grant, consuming it.
    ///
    /// Terminal: a committed reservation cannot be released or expired.
    async fn commit(&self, grant: Grant) -> Result<Reservation, LedgerError>;

    /// Release a previously issued grant, consuming it.
    ///
    /// The capacity returns to the available pool immediately (before lease
    /// expiry).
    async fn release(&self, grant: Grant) -> Result<Reservation, LedgerError>;

    /// Read the aggregate exposure for `resource`.
    ///
    /// `committed` and `outstanding` are reported separately (never summed).
    async fn exposure(&self, resource: &ResourceId) -> Result<Exposure, LedgerError>;
}
