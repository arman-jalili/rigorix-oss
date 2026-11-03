//! The six capability adapters, all fixture-driven.
//!
//! @canonical .pi/architecture/modules/portable-session.md#infrastructure-layer-infrastructure
//! Implements: Contract Freeze — the six adapters
//! Issue: #1294 (contract freeze)
//!
//! # Anti-corruption note
//!
//! There is no vendor SDK behind these adapters, so the ACL is thin: a transcript is
//! read into engine-shaped observations and the engine's vocabulary is preserved
//! verbatim (`passed`/`failed`/`error`). The translation that must not leak inward is
//! the *transcript's* shape — its keys never appear above this layer.

mod fixture_clock;
mod fixture_provider_transport;
mod fixture_randomness;
mod memory_business_sink;
mod seeded_id_source;
mod seeded_key_source;

pub use fixture_clock::FixtureClock;
pub use fixture_provider_transport::FixtureProviderTransport;
pub use fixture_randomness::FixtureRandomness;
pub use memory_business_sink::MemoryBusinessSink;
pub use seeded_id_source::SeededIdSource;
pub use seeded_key_source::SeededKeySource;
