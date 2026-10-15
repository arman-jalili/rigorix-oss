//! Infrastructure layer for the Capacity Ledger bounded context.
//!
//! @canonical .pi/architecture/modules/capacity-ledger.md#ddd-layers
//! Implements: ISSUE-CONTRACT-FREEZE — repository interface + adapter stubs
//! Issue: #1021
//!
//! Adapters implement the `CapacityLedger` port. They are the only place where
//! storage or transport types may appear; the domain and application layers
//! depend on nothing here.
//!
//! # Guarantee ceiling
//! `InProcessLedger` gives single-execution atomicity only. `HttpLedger` is the
//! only adapter that can provide cross-session atomicity, and only against a
//! real contested store.

pub mod http;
pub mod in_process;
pub mod repository;

pub use http::{HttpLedger, HttpLedgerConfig};
pub use in_process::InProcessLedger;
