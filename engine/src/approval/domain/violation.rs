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

impl ScopeViolation {
    /// Compute the effects outside the declared scope (pure path-set logic).
    ///
    /// A declared entry covers: the exact path (`src/auth.ts`) or every path
    /// under a directory entry (`src/` → `src/auth.ts`, `src/lib.rs`). The
    /// directory match is boundary-aware so `src` never swallows `src2/…`.
    pub fn out_of_scope(declared: &[String], actual: &[String]) -> Vec<String> {
        let declared: Vec<String> = declared
            .iter()
            .map(|d| d.trim_end_matches('/').to_string())
            .collect();
        let mut out = Vec::new();
        'effect: for effect in actual {
            let effect = effect.trim_end_matches('/');
            for entry in &declared {
                let covered = effect == entry || effect.starts_with(&format!("{entry}/"));
                if covered {
                    continue 'effect;
                }
            }
            out.push(effect.to_string());
        }
        out
    }

    /// Build a `ScopeViolation` from oracle effects vs the declared scope.
    ///
    /// Returns `None` when every recorded effect stayed inside the declared
    /// scope — in-scope execution is not a violation. Used by the post-
    /// execution effect-scope check (R5); the result is non-blocking,
    /// first-class envelope evidence.
    pub fn detect(
        node_id: Uuid,
        step_name: String,
        declared_scope: &[String],
        actual_effects: &[String],
        detected_at: DateTime<Utc>,
    ) -> Option<ScopeViolation> {
        let out_of_scope = Self::out_of_scope(declared_scope, actual_effects);
        if out_of_scope.is_empty() {
            return None;
        }
        Some(ScopeViolation {
            node_id,
            step_name,
            declared_scope: declared_scope.to_vec(),
            actual_effects: actual_effects.to_vec(),
            out_of_scope,
            detected_at,
        })
    }
}
