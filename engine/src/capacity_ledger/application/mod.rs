//! Application layer for the Capacity Ledger bounded context.
//!
//! @canonical .pi/architecture/modules/capacity-ledger.md#ddd-layers
//! Implements: ISSUE-CONTRACT-FREEZE — service, guard, DTOs, factory interfaces
//! Issue: #1021
//!
//! Defines the use cases over the [`CapacityLedger`] port, the RAII
//! `ReservationGuard`, the input/output DTOs, and the adapter factory
//! interface. Concrete implementations live in later issues; this layer is the
//! frozen contract they must satisfy.
//!
//! [`CapacityLedger`]: crate::capacity_ledger::interfaces::CapacityLedger

pub mod dto;
pub mod factory;
pub mod guard;
pub mod service;
pub mod service_impl;

pub use dto::{
    CommitCapacityInput, CommitCapacityOutput, ExposureOutput, ExposureQuery, ReleaseCapacityInput,
    ReleaseCapacityOutput, ReserveCapacityInput, ReserveCapacityOutput,
};
pub use factory::{CapacityLedgerFactory, LedgerBackendConfig};
pub use guard::ReservationGuard;
pub use service::CapacityLedgerService;
pub use service_impl::CapacityLedgerServiceImpl;
