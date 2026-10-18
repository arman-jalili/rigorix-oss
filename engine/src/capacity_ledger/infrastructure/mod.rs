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
//!
//! # No local persistence port
//! Storage is the adapter's concern, not a separate repository seam. The
//! shipped adapters own their state (`InProcessLedger`) or delegate to the
//! enterprise service (`HttpLedger`); nothing in the OSS engine persists
//! capacity locally. The former `CapacityLedgerRepository` port had no
//! implementation and no caller, so it was removed (#1016).

pub mod factory_impl;
pub mod http;
pub mod in_process;

pub use factory_impl::DefaultCapacityLedgerFactory;
pub use http::{HttpLedger, HttpLedgerConfig};
pub use in_process::InProcessLedger;
