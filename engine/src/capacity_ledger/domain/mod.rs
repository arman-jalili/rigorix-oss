//! Domain types, invariants, and errors for the Capacity Ledger.
//!
//! @canonical .pi/architecture/modules/capacity-ledger.md#ddd-layers
//! Implements: ISSUE-CONTRACT-FREEZE — domain entities Resource, Reservation,
//!              Grant, Exposure, IdempotencyKey, LedgerError, LedgerEvent
//! Issue: #1021
//!
//! Pure types only: **zero framework imports**. This module is the frozen model
//! that every adapter and use case depends on. Storage types (rows, tables,
//! connection handles) are forbidden here and belong behind the repository
//! interface in `infrastructure/`.

pub mod error;
pub mod event;
pub mod exposure;
pub mod grant;
pub mod idempotency;
pub mod reservation;
pub mod resource;

pub use error::LedgerError;
pub use event::LedgerEvent;
pub use exposure::Exposure;
pub use grant::Grant;
pub use idempotency::IdempotencyKey;
pub use reservation::{Lease, Reservation, ReservationId, ReservationState};
pub use resource::{ResourceId, ResourceSpec, Window};
