//! Domain errors — exhaustive, redacted, never an allow.
//!
//! @canonical .pi/architecture/modules/portable-session.md#error-handling
//! @canonical .pi/architecture/decisions/ADR-012-failure-semantics-on-wasm32.md
//! Implements: Contract Freeze — SessionOpenError, AttemptError, SealError
//! Issue: #1294 (contract freeze)
//!
//! # Contract (Frozen)
//! - Every variant is a **tier-1 logical failure**; a trap is not representable
//! - No variant carries a value from host data (no parameter, no path, no key)
//! - `Display` is business-readable; the interface layer maps each variant to a
//!   `RefusalDto`, so an error never escapes as an exception

mod attempt;
mod seal;
mod session_open;

pub use attempt::AttemptError;
pub use seal::SealError;
pub use session_open::SessionOpenError;
