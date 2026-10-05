//! GatingMode — `[gating]` step-outcome gating (ADR-017 §R2).
//!
//! @canonical .pi/architecture/modules/precondition.md#r2
//! @canonical .pi/architecture/modules/dag-engine.md#domain
//! @canonical .pi/architecture/decisions/ADR-017-consequence-gating.md
//! Implements: Contract Freeze — GatingMode (`release_dependents_on_failure`)
//! Issue: #938 (consequence-gating epic — contract freeze); behavior closed in
//!   ISSUE-CONSEQUENCE-GATING-7 (GatingMode); relocated to the DAG engine domain
//!   by ISSUE-PF-REL-1 (#972) — R2 is a graph/executor concern, not a
//!   precondition concern.
//!
//! Today the dispatch loop releases dependents unconditionally: a failed or
//! denied step does not stop what depends on it. `GatingMode` makes that a
//! deliberate operator choice.
//!
//! # Contract (Frozen)
//! - `release_dependents_on_failure` defaults to `true` — **today's behavior**
//!   (the flag is additive and opt-in; a missing `[gating]` table must not
//!   change any existing run)
//! - When `false`, a node whose terminal state is a failure (tool failure,
//!   ADR-013 sequence denial, or ADR-017 precondition denial) does **not**
//!   release its transitive dependents: they are marked `Skipped` and never
//!   dispatched, and the run reports the failure
//! - The mode is a **domain value**, not a runtime branch: the release-dependents
//!   logic in `execution_engine/application/service_impl/dispatch.rs` reads it
//!   and decides deterministically

use serde::{Deserialize, Serialize};

/// Step-outcome gating configuration (`[gating]` in
/// `.rigorix/preconditions.toml`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct GatingMode {
    /// When `true` (default), a failed/denied node releases its dependents
    /// (status quo — the DAG continues). When `false`, the failed node's
    /// transitive dependents are marked `Skipped` and never dispatched.
    #[serde(default = "default_release_dependents_on_failure")]
    pub release_dependents_on_failure: bool,
}

/// Default `release_dependents_on_failure` — `true`, preserving today's
/// behavior for configs that omit the `[gating]` table.
pub const fn default_release_dependents_on_failure() -> bool {
    true
}

impl Default for GatingMode {
    fn default() -> Self {
        Self {
            release_dependents_on_failure: default_release_dependents_on_failure(),
        }
    }
}

impl GatingMode {
    /// Whether a failed/denied node may release its transitive dependents.
    ///
    /// `true` = today's behavior; `false` = dependents are skipped.
    pub fn releases_dependents_on_failure(&self) -> bool {
        self.release_dependents_on_failure
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_to_releasing_dependents() {
        // Additive/opt-in: absent `[gating]` must preserve today's behavior.
        assert!(GatingMode::default().release_dependents_on_failure);
        assert!(GatingMode::default().releases_dependents_on_failure());
    }

    #[test]
    fn absent_field_deserializes_to_true() {
        let mode: GatingMode = serde_json::from_value(serde_json::json!({})).expect("empty table");
        assert!(mode.release_dependents_on_failure);
    }

    #[test]
    fn false_round_trips() {
        let mode = GatingMode {
            release_dependents_on_failure: false,
        };
        let encoded = serde_json::to_string(&mode).expect("serialize");
        let decoded: GatingMode = serde_json::from_str(&encoded).expect("deserialize");
        assert_eq!(decoded, mode);
        assert!(!decoded.releases_dependents_on_failure());
    }
}
