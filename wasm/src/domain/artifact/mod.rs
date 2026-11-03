//! The sealed, stamped artifact set — the only durable output.
//!
//! @canonical .pi/architecture/decisions/ADR-010-artifact-format.md
//! @canonical .pi/architecture/decisions/ADR-013-who-verifies.md
//! Implements: Contract Freeze — ArtifactSet, VersionStamp
//! Issue: #1294 (contract freeze)
//!
//! The container holds **engine-produced bytes** plus metadata this crate owns. It is
//! sealed once and never mutated. There is deliberately no verification type here: the
//! host verifies with `rigorix-verifier` (ADR-013), so the three-valued vocabulary is a
//! host obligation, not a crate type.

mod artifact_set;
mod version_stamp;

pub use artifact_set::{ArtifactSet, EnvelopeBytes};
pub use version_stamp::VersionStamp;
