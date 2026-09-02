//! ScopeViolation — first-class evidence that recorded effects exceeded the
//! approved scope (R5, effect-scope verification).
//!
//! @canonical .pi/architecture/modules/approval.md#scopeviolation
//! Implements: Contract Freeze — ScopeViolation evidence record
//! Issue: #786 (approval epic — contract freeze); behavior in
//!   ISSUE-SCOPEVIOLATION (#791)
//!
//! # Contract (Frozen)
//! - Declared scope (from R1) is compared against **recorded effects**
//! - The effect oracle is **git diff**: the engine snapshots `git status` /
//!   `git diff --stat` at dispatch (post-approval, pre-execution) and
//!   post-execution; the actual changed path set is the oracle. Engine-visible
//!   `file_paths` alone would miss side-effects from `run_command` scripts —
//!   git diff does not
//! - Effects outside the declared scope produce a `scope_violation` flag in
//!   the envelope (**non-blocking, first-class evidence** — R2 is the blocking
//!   check)
//! - The honest boundary is documented: a side-effect on `src/auth.ts` via a
//!   script can still happen — it is now *detected and recorded* as a
//!   violation in the signed record

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Recorded effect outside the declared scope (post-execution evidence).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScopeViolation {
    /// Node whose execution produced the out-of-scope effects.
    pub node_id: Uuid,
    /// Name of the step that was approved and executed.
    pub step_name: String,
    /// Approved effect scope (from the intent captured at approval time).
    pub declared_scope: Vec<String>,
    /// Actual changed path set, from the git-diff oracle.
    pub actual_effects: Vec<String>,
    /// Effects present in `actual_effects` but outside `declared_scope`.
    pub out_of_scope: Vec<String>,
    /// When the violation was detected (post-execution check).
    pub detected_at: DateTime<Utc>,
}
