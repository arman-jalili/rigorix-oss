//! Infrastructure — the fixture-backed adapters and the boundary helpers.
//!
//! @canonical .pi/architecture/modules/portable-session.md#infrastructure-layer-infrastructure
//! @canonical .pi/architecture/decisions/ADR-002-data-storage.md
//! @canonical .pi/architecture/decisions/ADR-013-who-verifies.md
//! Implements: Contract Freeze — infrastructure layer
//! Issue: #1294 (contract freeze)
//!
//! Every adapter answers from the fixture transcript or fails with a typed
//! `PortFailure`. None of them reaches the network, the filesystem, the OS clock or an
//! entropy source — that is the whole point of the layer.
//!
//! # Contract (Frozen)
//! - One adapter per capability port; the vendor/OSS choice is "none", because there is
//!   no vendor to call
//! - No verification logic and no verifier dependency (ADR-013, criterion 14)
//! - No persistence beyond an in-memory session store (ADR-002)

pub mod adapters;
pub mod canonical_serializer;
pub mod content_policy;
pub mod repository;
pub mod seam_gate;

pub use adapters::{
    FixtureClock, FixtureProviderTransport, FixtureRandomness, MemoryBusinessSink, SeededIdSource,
    SeededKeySource,
};
pub use canonical_serializer::CanonicalSerializer;
pub use content_policy::ScanningContentPolicy;
pub use repository::InMemorySessionStore;
pub use seam_gate::SeamGate;
