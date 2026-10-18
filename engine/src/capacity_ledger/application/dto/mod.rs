//! Data Transfer Objects for the Capacity Ledger.
//!
//! @canonical .pi/architecture/modules/capacity-ledger.md#capacityledgerservice
//! Implements: ISSUE-CONTRACT-FREEZE — reserve/commit/release/exposure DTO schemas
//! Issue: #1021
//!
//! DTOs define the input/output contracts for service and HTTP operations. They
//! carry documented validation metadata but no behavior.
//!
//! # Contract (Frozen)
//! - Every operation has a dedicated input and output DTO
//! - DTOs are JSON-serializable (`snake_case` fields as written)
//! - `committed` and `outstanding` are never collapsed in an output
//! - No DTO hands out a reservation handle: the service owns outstanding guards

use serde::{Deserialize, Serialize};

use crate::capacity_ledger::domain::{
    Exposure, IdempotencyKey, Reservation, ReservationId, ResourceId,
};

// ---------------------------------------------------------------------------
// Reserve
// ---------------------------------------------------------------------------

/// Input for reserving capacity.
///
/// # Fields (Frozen)
/// - `resource: ResourceId` — target resource label
/// - `amount: u64` — units to hold; must be `> 0`
/// - `idempotency_key: IdempotencyKey` — retry identity; reuse verbatim on retry
/// - `requested_by: Option<String>` — optional caller/step label for diagnostics
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReserveCapacityInput {
    /// Target resource label.
    pub resource: ResourceId,

    /// Units to hold (`> 0`).
    pub amount: u64,

    /// Retry identity; reuse verbatim on retry.
    pub idempotency_key: IdempotencyKey,

    /// Optional caller/step label for diagnostics.
    pub requested_by: Option<String>,
}

/// Output from a successful reservation.
///
/// # Fields (Frozen)
/// - `reservation: Reservation` — the resulting reservation snapshot
/// - `exposure: Exposure` — exposure after the reserve
///
/// The service owns the outstanding [`ReservationGuard`] internally and settles
/// it by reservation id through `commit`/`release`. No handle is handed out: a
/// caller cannot hold a guard while the service also owns its grant.
///
/// [`ReservationGuard`]: super::guard::ReservationGuard
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReserveCapacityOutput {
    /// The resulting reservation snapshot.
    pub reservation: Reservation,

    /// Exposure after the reserve.
    pub exposure: Exposure,
}

// ---------------------------------------------------------------------------
// Commit
// ---------------------------------------------------------------------------

/// Input for committing a reservation.
///
/// # Fields (Frozen)
/// - `reservation_id: ReservationId` — the reservation to consume
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommitCapacityInput {
    /// The reservation to consume.
    pub reservation_id: ReservationId,
}

/// Output from a commit.
///
/// # Fields (Frozen)
/// - `reservation: Reservation` — terminal (committed) snapshot
/// - `exposure: Exposure` — exposure after the commit
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommitCapacityOutput {
    /// Terminal (committed) snapshot.
    pub reservation: Reservation,

    /// Exposure after the commit.
    pub exposure: Exposure,
}

// ---------------------------------------------------------------------------
// Release
// ---------------------------------------------------------------------------

/// Input for releasing a reservation.
///
/// # Fields (Frozen)
/// - `reservation_id: ReservationId` — the reservation to hand back
/// - `reason: Option<String>` — optional human-readable release reason
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseCapacityInput {
    /// The reservation to hand back.
    pub reservation_id: ReservationId,

    /// Optional release reason.
    pub reason: Option<String>,
}

/// Output from a release.
///
/// # Fields (Frozen)
/// - `reservation: Reservation` — terminal (released/expired) snapshot
/// - `exposure: Exposure` — exposure after the release
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseCapacityOutput {
    /// Terminal (released/expired) snapshot.
    pub reservation: Reservation,

    /// Exposure after the release.
    pub exposure: Exposure,
}

// ---------------------------------------------------------------------------
// Exposure
// ---------------------------------------------------------------------------

/// Input for querying exposure.
///
/// # Fields (Frozen)
/// - `resource: ResourceId` — the resource to measure
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExposureQuery {
    /// The resource to measure.
    pub resource: ResourceId,
}

/// Output from an exposure query.
///
/// # Fields (Frozen)
/// - `exposure: Exposure` — `committed` and `outstanding` reported separately
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExposureOutput {
    /// The snapshot; `committed` and `outstanding` stay distinct.
    pub exposure: Exposure,
}
