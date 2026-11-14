//! The receipt module's infrastructure layer: where an attestation is written, and why there.
//!
//! @canonical .pi/architecture/modules/receipt.md
//! Implements: ADR-024 §6 (E4 — the seal and where it lives)
//! Issue: #1527
//!
//! # The location rule, and the correction this module carries
//!
//! This module used to say an attestation would be persisted "beside the audit envelope, in the same
//! repository, as a new record: the envelope already owns the durable, append-only store". That was
//! wrong, and it was wrong in the one place the ADR is explicit about: `<repo>/.rigorix/audit` is
//! inside the tree the agent's write tools are contained to, so a seal stored there is a seal the
//! agent can lower before it is evaluated (ADR-024 §6: "a seal inside the agent-writable tree is not
//! a seal").
//!
//! The envelope store is also ADR-016's `local_unanchored` regime — reachable, by design, from the
//! cross-run policy history. A signed observation of what a run did has a different job, so it lives
//! somewhere else:
//!
//! ```text
//! <state dir>/attestations/<execution_id>.json
//! ```
//!
//! `<state dir>` is [`crate::receipt::application::default_state_dir`]'s answer — `RIGORIX_STATE_DIR`
//! when the operator set one, otherwise `<repo>/../.execution-state/<slug>`, a sibling of the
//! repository and outside every workspace by construction. The envelope carries the attestation's
//! **digest** (a reference) and never a copy of it: one sealed record, one home, no second trail.
//!
//! # Why the module has an adapter now
//!
//! Because the location rule is a property of the *store*, not of a caller. `FileAttestationStore`
//! is the port that lets the orchestrator write the record without deciding where it goes, and it
//! refuses a state directory inside the workspace rather than trusting whoever constructed it.

pub mod attestation_store;

pub use attestation_store::{AttestationStoreError, FileAttestationStore, StoredAttestation};
