//! RiskConfig domain entity — configurable risk policies.
//!
//! @canonical .pi/architecture/modules/risk-gating.md#config
//! Implements: Contract Freeze — RiskConfig entity
//! Issue: issue-contract-freeze
//!
//! Defines configurable overrides and gating behaviors for the risk-gating
//! module. The RiskConfig is loaded from the application configuration and
//! used by the RiskClassifier to determine per-tool risk levels and gating
//! policies.
//!
//! # Contract (Frozen)
//! - `RiskConfig` is the root aggregate for all risk-gating configuration
//! - Loaded from `Config.risk_gating` section
//! - All fields are public for direct access by application services
//! - `tool_overrides` map is checked by the classifier before default rules

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::risk_gating::domain::risk_level::RiskLevel;

/// Configuration for risk-gating behavior.
///
/// Defines per-tool risk overrides and global gating policy flags.
/// This is typically loaded from the `[risk_gating]` section of the
/// application configuration file (e.g., `rigorix.toml`).
///
/// # Example (TOML)
///
/// ```toml
/// [risk_gating]
/// # Override default risk levels for specific tools
/// tool_overrides = { "run_command" = "high", "git_push" = "high" }
///
/// # Gate behavior
/// auto_confirm_low = true
/// require_review_medium = true
/// dry_run_high = true
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RiskConfig {
    /// Per-tool risk level overrides keyed by tool name.
    ///
    /// If a tool is listed here, its configured risk level takes
    /// precedence over the default classification rules.
    /// Example: `{ "run_command" = "high", "git_commit" = "high" }`
    pub tool_overrides: HashMap<String, RiskLevel>,

    /// Whether Low-risk tools should auto-execute without any gate.
    /// Default: `true`.
    pub auto_confirm_low: bool,

    /// Whether Medium-risk tools require explicit user review/confirmation.
    /// Default: `true`.
    pub require_review_medium: bool,

    /// Whether High-risk tools should execute in dry-run mode by default.
    /// Default: `true`.
    pub dry_run_high: bool,
}

impl Default for RiskConfig {
    fn default() -> Self {
        Self {
            tool_overrides: HashMap::new(),
            auto_confirm_low: true,
            require_review_medium: true,
            dry_run_high: true,
        }
    }
}

impl RiskConfig {
    /// Create a new `RiskConfig` with the given overrides and default gating flags.
    pub fn new(tool_overrides: HashMap<String, RiskLevel>) -> Self {
        Self {
            tool_overrides,
            ..Self::default()
        }
    }

    /// Get the override risk level for a specific tool, if one exists.
    pub fn get_override(&self, tool_name: &str) -> Option<&RiskLevel> {
        self.tool_overrides.get(tool_name)
    }

    /// Check if a tool has a configured override.
    pub fn has_override(&self, tool_name: &str) -> bool {
        self.tool_overrides.contains_key(tool_name)
    }

    /// Add or update a tool override.
    pub fn set_override(&mut self, tool_name: String, risk_level: RiskLevel) {
        self.tool_overrides.insert(tool_name, risk_level);
    }

    /// Remove a tool override, restoring default classification.
    pub fn remove_override(&mut self, tool_name: &str) -> Option<RiskLevel> {
        self.tool_overrides.remove(tool_name)
    }

    /// Merge another RiskConfig into this one.
    ///
    /// Overrides from `other` take precedence for conflicting keys.
    /// Gating flags are overwritten by `other` values.
    pub fn merge(&mut self, other: RiskConfig) {
        self.tool_overrides.extend(other.tool_overrides);
        self.auto_confirm_low = other.auto_confirm_low;
        self.require_review_medium = other.require_review_medium;
        self.dry_run_high = other.dry_run_high;
    }
}
