//! Infrastructure layer for Effect Commitment.
//!
//! @canonical .pi/architecture/modules/effect-commitment.md#ddd-layers
//! Implements: ISSUE-CONTRACT-FREEZE — adapters + concrete factory
//! Issue: #1113
//!
//! Adapters implement the `EffectCommitmentLedger` port. They are the only place
//! where storage or transport types may appear; the domain and application
//! layers depend on nothing here.
//!
//! # Guarantee ceiling
//! `InMemoryEffectLedger` gives single-process durability (L1).
//! `HttpEffectLedger` is the only adapter that can provide cross-process
//! settlement (L2), and only against a real contested store.
//! `HttpProviderReceiptReader` is the L3 boundary; without it the context can
//! prove **not-settled** and never **settled**.
//!
//! # Contract Freeze Notice
//! The adapters below are typed stubs that fail closed with
//! `CommitmentError::Unavailable`. Implementation issues replace the bodies
//! without changing these signatures.

pub mod factory_impl;
pub mod http;
pub mod in_memory;
pub mod provider_receipt;

pub use factory_impl::DefaultEffectCommitmentFactory;
pub use http::{HttpEffectLedger, HttpEffectLedgerConfig};
pub use in_memory::InMemoryEffectLedger;
pub use provider_receipt::HttpProviderReceiptReader;
