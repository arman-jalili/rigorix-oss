//! Interfaces layer for the Capacity Ledger bounded context.
//!
//! @canonical .pi/architecture/modules/capacity-ledger.md#ddd-layers
//! Implements: ISSUE-CONTRACT-FREEZE — `CapacityLedger` port + HTTP contracts
//! Issue: #1021
//!
//! `interfaces/` is the contract consumed by `enforcement`. It holds the
//! `CapacityLedger` port (the model the engine programs against) and the HTTP
//! endpoint contracts exposed by the opt-in distributed adapter.

pub mod capacity_ledger;
#[cfg(test)]
pub mod contract;
pub mod http;

pub use capacity_ledger::CapacityLedger;
