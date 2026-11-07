//! The run's result: status, ordered steps, evidence references.
//!
//! @canonical .pi/architecture/modules/portable-session.md#value-objects-entity-vs-vo-justification
//! Implements: Contract Freeze — RunOutcome, StepOutcome, OutcomeStatus, StepDecision,
//!   RefusalKind, EvidenceRef, EvidenceKind
//! Issue: #1294 (contract freeze)
//!
//! There are exactly two run statuses and three step decisions. That is not an
//! oversight — a third run status is how a host-settable verdict sneaks in, and
//! collapsing `NotReached` into `Refused` is how a record stops explaining itself.

mod evidence;
mod refusal;
mod run_outcome;
mod step_outcome;

pub use evidence::{EvidenceKind, EvidenceRef};
pub use refusal::RefusalKind;
pub use run_outcome::{OutcomeStatus, RunOutcome};
pub use step_outcome::{RefusalDetail, StepDecision, StepOutcome};
