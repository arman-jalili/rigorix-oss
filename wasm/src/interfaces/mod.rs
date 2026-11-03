//! Interfaces — the published host contract.
//!
//! @canonical .pi/architecture/decisions/ADR-004-api-design.md
//! @canonical .pi/architecture/decisions/ADR-013-who-verifies.md
//! Implements: Contract Freeze — interfaces layer
//! Issue: #1294 (contract freeze)
//!
//! Four functions, JSON in and JSON out. That is the entire surface a host programs
//! against.
//!
//! # The host's verification obligation (ADR-013)
//!
//! The crate produces the record; the **host** verifies it with `rigorix-verifier` in
//! JavaScript, in a process that has never loaded this module. The host renders exactly
//! three values and never collapses them:
//!
//! | Value | Meaning | Must not be shown as |
//! |-------|---------|----------------------|
//! | `verified` | compared and matches | "trusted", "safe", "approved" |
//! | `not_verified` | compared and does not match | "unverifiable" |
//! | `unverifiable(reason)` | could not compare (unsupported format, absent key, key not held) | "not_verified", "tampered", "unauthenticated" |
//!
//! There is deliberately no verification type and no verifier dependency in this
//! crate: a verifier compiled into the producer's own module would make the demo's
//! central sentence false (criterion 12, 14).
//!
//! # Contract (Frozen)
//! - No function accepts a verdict, an allow flag or an override (FR-009)
//! - A refusal is a return value, never a thrown error; only a malformed handle throws
//!   (ADR-012)
//! - No `console` output, no logging from the crate

pub mod dto;
pub mod errors;
pub mod events;
pub mod registry;
pub mod wasm_api;

pub use dtos::*;
pub use errors::RefusalDto;
pub use events::{Observed, TelemetryObserver};
pub use wasm_api::{
    CONTRACT_FUNCTIONS, ContractVersion, attempt_run, contract_version, export_artifact_set,
    open_session,
};

mod dtos {
    //! Re-export shim so `dto` items are reachable as `interfaces::*`.
    pub use super::dto::*;
}
