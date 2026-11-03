//! The six capability ports, the domain-owned session store, and the port bundle.
//!
//! @canonical .pi/architecture/modules/portable-session.md#ports-declared-by-the-domain
//! @canonical .pi/architecture/decisions/ADR-002-data-storage.md
//! @canonical .pi/architecture/decisions/ADR-012-failure-semantics-on-wasm32.md
//! Implements: Contract Freeze — Clock, IdSource, Randomness, KeySource,
//!   ProviderTransport, BusinessSink, SessionStore, Ports
//! Issue: #1294 (contract freeze)
//!
//! The engine declares that it needs these capabilities; the domain owns the traits;
//! `infrastructure` implements them from a fixture transcript. Nothing else in the
//! crate may reach a capability — that is what invariant 1 means in code.
//!
//! # Contract (Frozen)
//! - All ports are **synchronous**: `wasm32-unknown-unknown` is single-threaded and
//!   the crate carries no async runtime (NFR-002)
//! - All ports are **trait-object safe** and take `&mut self` (a port answers once
//!   per call; a transcript is consumed, not queried)
//! - Every method returns `Result<_, PortFailure>`; there is no `Option` and no
//!   default value, so "no answer" cannot be read as "fine" (invariant 2)

mod business_sink;
mod clock;
mod id_source;
mod key_source;
mod provider_transport;
mod randomness;
mod session_store;

pub use business_sink::{BusinessSink, SinkWrite};
pub use clock::Clock;
pub use id_source::{IdKind, IdSource};
pub use key_source::KeySource;
pub use provider_transport::{Determination, ProviderCall, ProviderObservation, ProviderTransport};
pub use randomness::Randomness;
pub use session_store::SessionStore;

use crate::domain::value::PortFailure;

/// The set of capability ports a single run may consult.
///
/// # Contract (Frozen)
/// - Passed as one bundle so a run cannot be handed a capability twice, or a
///   different clock mid-run
/// - Borrowing is exclusive: the bundle is consumed by one attempt
pub struct Ports<'a> {
    /// Time.
    pub clock: &'a mut dyn Clock,
    /// Identifiers.
    pub ids: &'a mut dyn IdSource,
    /// Entropy.
    pub randomness: &'a mut dyn Randomness,
    /// Key reference.
    pub key_source: &'a dyn KeySource,
    /// Operator check transport.
    pub provider: &'a mut dyn ProviderTransport,
    /// Business sink.
    pub sink: &'a mut dyn BusinessSink,
}

impl std::fmt::Debug for Ports<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Ports").finish_non_exhaustive()
    }
}

/// Convenience alias for a port result.
pub type PortResult<T> = Result<T, PortFailure>;

/// The store a sealed session is handed to (ADR-002: in memory, no database).
///
/// An alias, not a second trait: the interface has exactly one definition.
pub type SessionRepository = dyn SessionStore;
