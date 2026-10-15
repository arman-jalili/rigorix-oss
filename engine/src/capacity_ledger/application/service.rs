//! CapacityLedgerService use-case contract.
//!
//! @canonical .pi/architecture/modules/capacity-ledger.md#capacityledgerservice
//! Implements: ISSUE-CONTRACT-FREEZE — CapacityLedgerService service interface
//! Issue: #1021
//!
//! The use cases over the port: reserve, commit, release, exposure. Contains no
//! storage logic and no local counter — a local counter is precisely the
//! naive-counter failure this module exists to replace.
//!
//! # Contract (Frozen)
//! - Every use case takes a DTO and returns a DTO or a typed `LedgerError`
//! - `reserve` returns a RAII guard that releases on drop (ADR-008)
//! - The service owns no storage; it delegates to a `CapacityLedger` adapter

use async_trait::async_trait;

use crate::capacity_ledger::domain::LedgerError;

use super::dto::{
    CommitCapacityInput, CommitCapacityOutput, ExposureOutput, ExposureQuery, ReleaseCapacityInput,
    ReleaseCapacityOutput, ReserveCapacityInput, ReserveCapacityOutput,
};

/// Application service exposing capacity-ledger use cases.
#[async_trait]
pub trait CapacityLedgerService: Send + Sync {
    /// Reserve capacity and return an RAII guard.
    ///
    /// # Errors
    /// - [`LedgerError::Insufficient`] when the cap is reached
    /// - [`LedgerError::Unavailable`] when the adapter cannot be reached
    async fn reserve(
        &self,
        input: ReserveCapacityInput,
    ) -> Result<ReserveCapacityOutput, LedgerError>;

    /// Commit a reservation, consuming its capacity.
    async fn commit(&self, input: CommitCapacityInput)
    -> Result<CommitCapacityOutput, LedgerError>;

    /// Release a reservation before consumption.
    async fn release(
        &self,
        input: ReleaseCapacityInput,
    ) -> Result<ReleaseCapacityOutput, LedgerError>;

    /// Read the aggregate exposure for a resource.
    async fn exposure(&self, input: ExposureQuery) -> Result<ExposureOutput, LedgerError>;
}
