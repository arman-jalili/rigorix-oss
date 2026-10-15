//! InProcessLedger adapter stub.
//!
//! @canonical .pi/architecture/modules/capacity-ledger.md#inprocessledger
//! Implements: ISSUE-CONTRACT-FREEZE — InProcessLedger adapter contract
//! Issue: #1021
//!
//! The standalone default. Single-execution atomicity via the proven ADR-008
//! discipline (shared state + RAII). No database, no network.
//!
//! # Guarantee ceiling (Frozen)
//! This adapter provides **single-execution atomicity only**. It MUST NOT be
//! described as "the distributed ledger"; cross-session atomicity requires the
//! HTTP adapter against a real contested store.
//!
//! # Status
//! The port methods are TODO stubs that fail closed with
//! [`LedgerError::Unavailable`]; ISSUE-CAPACITY-LEDGER-9 supplies the real
//! implementation. The type and its constructor are frozen here so wiring and
//! contract tests can target it.

use async_trait::async_trait;

use crate::capacity_ledger::domain::{
    Exposure, Grant, IdempotencyKey, LedgerError, Reservation, ResourceId, ResourceSpec,
};
use crate::capacity_ledger::interfaces::CapacityLedger;

/// The in-process capacity-ledger adapter.
pub struct InProcessLedger {
    _private: (),
}

impl InProcessLedger {
    /// Create an empty in-process ledger.
    pub fn new() -> Self {
        Self { _private: () }
    }
}

impl Default for InProcessLedger {
    fn default() -> Self {
        Self::new()
    }
}

/// Fail-closed placeholder body. TODO(ISSUE-CAPACITY-LEDGER-9): replace each
/// call with the real in-process operation.
fn not_implemented(op: &str) -> LedgerError {
    LedgerError::Unavailable {
        detail: format!("InProcessLedger::{op} not implemented yet (ISSUE-CAPACITY-LEDGER-9)"),
    }
}

#[async_trait]
impl CapacityLedger for InProcessLedger {
    async fn declare_resource(&self, _spec: ResourceSpec) -> Result<(), LedgerError> {
        Err(not_implemented("declare_resource"))
    }

    async fn reserve(
        &self,
        _resource: &ResourceId,
        _amount: u64,
        _key: IdempotencyKey,
    ) -> Result<Grant, LedgerError> {
        Err(not_implemented("reserve"))
    }

    async fn commit(&self, _grant: Grant) -> Result<Reservation, LedgerError> {
        Err(not_implemented("commit"))
    }

    async fn release(&self, _grant: Grant) -> Result<Reservation, LedgerError> {
        Err(not_implemented("release"))
    }

    async fn exposure(&self, _resource: &ResourceId) -> Result<Exposure, LedgerError> {
        Err(not_implemented("exposure"))
    }
}
