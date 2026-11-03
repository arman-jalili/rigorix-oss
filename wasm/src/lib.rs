//! `rigorix-gate-wasm` — the portable Rigorix gate seam.
//!
//! @canonical .pi/architecture/modules/portable-session.md
//! @canonical .pi/architecture/decisions/ADR-001-architecture-pattern.md
//! @canonical .pi/architecture/decisions/ADR-011-crate-identity-and-release-train.md
//! @canonical .pi/architecture/decisions/ADR-012-failure-semantics-on-wasm32.md
//! @canonical .pi/architecture/decisions/ADR-013-who-verifies.md
//! Implements: Contract Freeze — portable-session module root
//! Issue: #1294 (contract freeze)
//!
//! # What this crate is
//!
//! It compiles a slice of the Rigorix engine to WebAssembly so a host application
//! can run the **real** gate locally: evaluate whether an AI agent's proposed action
//! may proceed, refuse it with evidence when it may not, produce signed envelopes,
//! and export a record a third party can verify offline.
//!
//! It owns exactly one bounded context — **Portable Session** — and one guarantee:
//! the bytes it produces are the bytes the CLI produces for the same input.
//!
//! # Architecture
//!
//! ```text
//! src/
//! ├── lib.rs                  # module root ← this file
//! ├── error.rs                # GateError: root error, wraps the domain errors
//! ├── domain/                 # pure: invariants, value objects, ports, events
//! │   ├── value/              # 12 value objects (no identity, immutable)
//! │   ├── ports/              # 6 engine-declared capability ports + session store
//! │   ├── session/            # ReplaySession (aggregate root) + GateAttempt
//! │   ├── outcome/            # RunOutcome, StepOutcome, EvidenceRef
//! │   ├── artifact/           # ArtifactSet, VersionStamp (sealed, stamped)
//! │   ├── events/             # 7 domain events, evidence vs telemetry
//! │   ├── services/           # RefusalVocabulary, EvidenceProjection, ArtifactStamp
//! │   ├── errors/             # SessionOpenError, AttemptError, SealError
//! │   └── engine_seam.rs      # GateEvaluator port (engine binding, FR-011)
//! ├── application/            # 4 catalog services + read helper + DTOs
//! ├── infrastructure/         # fixture-backed adapters, scanner, serializer
//! └── interfaces/             # the published JSON host contract (4 functions)
//! ```
//!
//! **Dependency rule:** `interfaces → application → domain`, with `infrastructure`
//! implementing traits declared in `domain`. The `domain` layer names no
//! `std::time`, `std::env`, `std::fs`, no random source and no `infrastructure`
//! path — enforced by the `domain_isolation` test (criterion 18).
//!
//! # What this crate is not
//!
//! - It does not decide anything. Precondition evaluation, sequence/requirement
//!   policy, approval binding, envelope construction and signing, identity
//!   attestation are the engine's (shared kernel, ADR-001).
//! - It does not verify. The host verifies with `rigorix-verifier`; the crate has
//!   no verifier dependency and no verification types (ADR-013, criterion 14).
//! - It has no storage, no server, no HTTP and no metering (ADR-002, ADR-004,
//!   ADR-006).
//!
//! # Failure semantics (ADR-012)
//!
//! Three tiers, three different promises:
//!
//! | Tier | Kind | Promise |
//! |------|------|---------|
//! | 1 | logical — missing/exhausted/malformed fixture answer, unparseable scenario, unsupported format | typed **refusal**; never an allow |
//! | 2 | program defect — panic, unwrap on `None` | must not exist on the gate path; caught by baseline + deny-lints |
//! | 3 | trap — the module aborts | **fatal and host-visible**; never rendered as a refusal or an allow |
//!
//! There is deliberately no `catch_unwind`: `wasm32-unknown-unknown` has no
//! unwinder, so a design that "catches the panic and returns Refused" is written
//! for a platform that does not exist.
//!
//! # Contract Freeze Notice
//!
//! Every public item in `domain/` and `application/` is a **frozen contract**.
//! Implementation issues satisfy these signatures; they do not edit them.
//! The gate path carries no `todo!()`: a stub that panics would trap the module
//! (ADR-012 tier 3), so unimplemented behaviour refuses instead.

pub mod application;
pub mod domain;
pub mod error;
pub mod infrastructure;
pub mod interfaces;
pub mod portable_session;

pub use error::GateError;
