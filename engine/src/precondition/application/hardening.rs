//! HardeningConfig — `max_failures_before_abort` reachable from `rigorix.toml`
//! and fail-closed arming (ADR-017 §Phase C).
//!
//! @canonical .pi/architecture/modules/precondition.md#fail-modes
//! @canonical .pi/architecture/decisions/ADR-017-consequence-gating.md
//! Implements: Contract Freeze — HardeningConfig
//! Issue: #938 (consequence-gating epic — contract freeze); behavior closed in
//!   ISSUE-CONSEQUENCE-GATING-11 (HardeningConfig)
//!
//! Today `max_failures_before_abort` is `0 = unlimited`
//! (`execution_engine/domain/parallel_executor.rs`) and hardcoded to `0` at
//! both composition roots (`cli/src/cli_boundary/orchestrator.rs`,
//! `actions/src/main.rs`); MCP uses `Default`. It is not config-reachable from
//! `rigorix.toml`.
//!
//! # Contract (Frozen)
//! - `max_failures_before_abort` defaults to `None` → effective `0` (unlimited,
//!   today's behavior). The type is **additive**: an omitted key must not
//!   change any existing run
//! - The field is threaded into the parallel executor's abort threshold by the
//!   composition roots; the executor's `0 = unlimited` sentinel is preserved
//! - Fail-closed arming: a consequential run that **expected** gating but could
//!   not arm preconditions is refused rather than silently degrading
//!   (`DispatchGate::is_armed` / `PreconditionError::NotArmed`)

use serde::{Deserialize, Serialize};

/// Hardening knobs exposed to `rigorix.toml`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct HardeningConfig {
    /// Maximum node failures before the run aborts. `None` (absent) → `0`
    /// (unlimited — today's behavior). Threaded into the parallel executor by
    /// the composition roots.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_failures_before_abort: Option<u32>,
}

impl HardeningConfig {
    /// The threshold handed to the parallel executor. `None` → `0` (the
    /// executor's `0 = unlimited` sentinel).
    pub fn effective_max_failures_before_abort(&self) -> u32 {
        self.max_failures_before_abort.unwrap_or(0)
    }

    /// Whether `failures` has reached the configured abort threshold.
    ///
    /// `None` / `Some(0)` → never aborts on failure count (unlimited).
    pub fn aborts_after(&self, failures: u32) -> bool {
        match self.max_failures_before_abort {
            None | Some(0) => false,
            Some(max) => failures >= max,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn absent_is_unlimited_today() {
        let config = HardeningConfig::default();
        assert_eq!(config.effective_max_failures_before_abort(), 0);
        assert!(!config.aborts_after(u32::MAX));
    }

    #[test]
    fn explicit_threshold_aborts() {
        let config = HardeningConfig {
            max_failures_before_abort: Some(3),
        };
        assert!(!config.aborts_after(2));
        assert!(config.aborts_after(3));
        assert!(config.aborts_after(4));
    }
}
