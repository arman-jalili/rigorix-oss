//! SequencePolicyConfig — the loaded rule set with safety caps.
//!
//! @canonical .pi/architecture/modules/sequence-policy.md#configuration
//! Implements: Contract Freeze — SequencePolicyConfig + SafetyCaps + validate()
//! Issue: #838 (sequence-policy epic — contract freeze); load/parse and cap
//!   enforcement in ISSUE-SEQUENCE-POLICY (fail-closed / fail-open-absent)
//!
//! Rules are declared in repository/org configuration
//! (`.rigorix/sequence-policy.toml`, same trust surface as `policy.toml` /
//! `permissions.toml`), authored by platform/security operators — **not** by
//! the executing agent (R5). The config is read per-run from disk.
//!
//! # Contract (Frozen)
//! - `fail_closed = true` (default): an unparseable / over-cap rule set blocks
//!   plan execution; a **missing** config file is `Ok(None)` (fail-open-absent
//!   — status quo, no gating) — never an error
//! - `rules` is the ordered rule set; empty rules → no matches
//! - `validate(&SafetyCaps)` enforces the safety caps (max rules per file, max
//!   predicates per rule, max window, max regex predicates — regex count is a
//!   denial-of-service surface); implementation in the fail-closed /
//!   fail-open-absent issues

use serde::{Deserialize, Serialize};

use super::error::SequencePolicyError;
use super::rule::SequenceRule;

/// Safety caps for the loaded rule set (mirrors `EnforcementConfig::validate`
/// posture). Regex predicates are capped because they are a ReDoS /
/// resource-exhaustion surface.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SafetyCaps {
    /// Maximum number of `[[rules]]` entries per config file.
    pub max_rules_per_file: u32,
    /// Maximum number of ordered step predicates per rule.
    pub max_steps_per_rule: u32,
    /// Maximum `window` gap between the first and last matched step.
    pub max_window: u32,
    /// Maximum number of regex parameter predicates across the file.
    pub max_regex_predicates_per_file: u32,
}

/// The loaded sequence-policy rule set.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SequencePolicyConfig {
    /// Fail closed on config errors at plan time. Defaults to `true` — a
    /// corrupt rule file must refuse the plan rather than silently degrade.
    #[serde(default = "default_fail_closed")]
    pub fail_closed: bool,
    /// The ordered rule set (empty → no sequence gating).
    #[serde(default)]
    pub rules: Vec<SequenceRule>,
}

const fn default_fail_closed() -> bool {
    true
}

impl Default for SequencePolicyConfig {
    fn default() -> Self {
        Self {
            fail_closed: true,
            rules: Vec::new(),
        }
    }
}

impl SequencePolicyConfig {
    /// Validate this config against the safety caps.
    ///
    /// Returns `Ok(())` when every rule is within the caps.
    ///
    /// # Errors
    /// - `SequencePolicyError::RuleExceedsCaps` — a rule exceeds one of the
    ///   caps (`rule` = rule id, `detail` = which cap and by how much)
    ///
    /// # Implementation
    /// TODO: enforced in the fail-closed / fail-open-absent issues (config
    /// load + parse land in `infrastructure/repository/toml_repository.rs`).
    pub fn validate(&self, _caps: &SafetyCaps) -> Result<(), SequencePolicyError> {
        todo!("ISSUE fail-closed/fail-open: enforce SafetyCaps over self.rules")
    }
}
