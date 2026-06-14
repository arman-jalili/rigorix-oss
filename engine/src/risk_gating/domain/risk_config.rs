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

    /// Create a strict configuration (all gates enabled).
    pub fn strict() -> Self {
        Self {
            tool_overrides: HashMap::new(),
            auto_confirm_low: true,
            require_review_medium: true,
            dry_run_high: true,
        }
    }

    /// Create a permissive configuration (all gates disabled).
    pub fn permissive() -> Self {
        Self {
            tool_overrides: HashMap::new(),
            auto_confirm_low: false,
            require_review_medium: false,
            dry_run_high: false,
        }
    }

    /// Create a custom configuration with all parameters.
    pub fn custom(
        tool_overrides: HashMap<String, RiskLevel>,
        auto_confirm_low: bool,
        require_review_medium: bool,
        dry_run_high: bool,
    ) -> Self {
        Self {
            tool_overrides,
            auto_confirm_low,
            require_review_medium,
            dry_run_high,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // Default
    // -----------------------------------------------------------------------

    #[test]
    fn test_default_config() {
        let config = RiskConfig::default();
        assert!(config.tool_overrides.is_empty());
        assert!(config.auto_confirm_low);
        assert!(config.require_review_medium);
        assert!(config.dry_run_high);
    }

    // -----------------------------------------------------------------------
    // Constructors
    // -----------------------------------------------------------------------

    #[test]
    fn test_new_with_overrides() {
        let mut overrides = HashMap::new();
        overrides.insert("bash".to_string(), RiskLevel::Low);
        let config = RiskConfig::new(overrides);

        assert_eq!(config.tool_overrides.len(), 1);
        assert_eq!(config.get_override("bash"), Some(&RiskLevel::Low));
        assert!(config.auto_confirm_low);
        assert!(config.require_review_medium);
        assert!(config.dry_run_high);
    }

    #[test]
    fn test_strict_config() {
        let config = RiskConfig::strict();
        assert!(config.auto_confirm_low);
        assert!(config.require_review_medium);
        assert!(config.dry_run_high);
        assert!(config.tool_overrides.is_empty());
    }

    #[test]
    fn test_permissive_config() {
        let config = RiskConfig::permissive();
        assert!(!config.auto_confirm_low);
        assert!(!config.require_review_medium);
        assert!(!config.dry_run_high);
    }

    #[test]
    fn test_custom_config() {
        let mut overrides = HashMap::new();
        overrides.insert("run_command".to_string(), RiskLevel::High);
        let config = RiskConfig::custom(overrides, false, true, false);

        assert!(!config.auto_confirm_low);
        assert!(config.require_review_medium);
        assert!(!config.dry_run_high);
        assert_eq!(config.tool_overrides.len(), 1);
    }

    // -----------------------------------------------------------------------
    // Override management
    // -----------------------------------------------------------------------

    #[test]
    fn test_get_override_exists() {
        let mut overrides = HashMap::new();
        overrides.insert("bash".to_string(), RiskLevel::High);
        let config = RiskConfig::new(overrides);

        assert_eq!(config.get_override("bash"), Some(&RiskLevel::High));
    }

    #[test]
    fn test_get_override_not_found() {
        let config = RiskConfig::default();
        assert_eq!(config.get_override("nonexistent"), None);
    }

    #[test]
    fn test_has_override_true() {
        let mut overrides = HashMap::new();
        overrides.insert("git_commit".to_string(), RiskLevel::High);
        let config = RiskConfig::new(overrides);

        assert!(config.has_override("git_commit"));
    }

    #[test]
    fn test_has_override_false() {
        let config = RiskConfig::default();
        assert!(!config.has_override("git_commit"));
    }

    #[test]
    fn test_set_override_new() {
        let mut config = RiskConfig::default();
        config.set_override("bash".to_string(), RiskLevel::Low);

        assert!(config.has_override("bash"));
        assert_eq!(config.get_override("bash"), Some(&RiskLevel::Low));
    }

    #[test]
    fn test_set_override_update() {
        let mut config = RiskConfig::default();
        config.set_override("bash".to_string(), RiskLevel::Low);
        config.set_override("bash".to_string(), RiskLevel::High);

        assert_eq!(config.get_override("bash"), Some(&RiskLevel::High));
    }

    #[test]
    fn test_remove_override_exists() {
        let mut config = RiskConfig::default();
        config.set_override("bash".to_string(), RiskLevel::High);

        let removed = config.remove_override("bash");
        assert_eq!(removed, Some(RiskLevel::High));
        assert!(!config.has_override("bash"));
    }

    #[test]
    fn test_remove_override_not_found() {
        let mut config = RiskConfig::default();
        let removed = config.remove_override("nonexistent");
        assert_eq!(removed, None);
    }

    #[test]
    fn test_multiple_overrides() {
        let mut overrides = HashMap::new();
        overrides.insert("bash".to_string(), RiskLevel::High);
        overrides.insert("file_read".to_string(), RiskLevel::Low);
        overrides.insert("file_write".to_string(), RiskLevel::Medium);
        let config = RiskConfig::new(overrides);

        assert_eq!(config.tool_overrides.len(), 3);
        assert_eq!(config.get_override("bash"), Some(&RiskLevel::High));
        assert_eq!(config.get_override("file_read"), Some(&RiskLevel::Low));
        assert_eq!(config.get_override("file_write"), Some(&RiskLevel::Medium));
    }

    // -----------------------------------------------------------------------
    // Merge
    // -----------------------------------------------------------------------

    #[test]
    fn test_merge_combines_overrides() {
        let mut base = RiskConfig::default();
        base.set_override("bash".to_string(), RiskLevel::High);

        let mut other = RiskConfig::default();
        other.set_override("file_write".to_string(), RiskLevel::Low);
        other.auto_confirm_low = false;

        base.merge(other);

        assert!(base.has_override("bash"));
        assert!(base.has_override("file_write"));
        assert!(!base.auto_confirm_low); // Overwritten by other
    }

    #[test]
    fn test_merge_other_overrides_take_precedence() {
        let mut base = RiskConfig::default();
        base.set_override("bash".to_string(), RiskLevel::Low);

        let mut other = RiskConfig::default();
        other.set_override("bash".to_string(), RiskLevel::High);

        base.merge(other);
        assert_eq!(base.get_override("bash"), Some(&RiskLevel::High));
    }

    #[test]
    fn test_merge_empty_other() {
        let mut base = RiskConfig::default();
        base.set_override("bash".to_string(), RiskLevel::High);

        let other = RiskConfig::default();
        base.merge(other);

        assert_eq!(base.get_override("bash"), Some(&RiskLevel::High));
        assert!(base.auto_confirm_low); // Not changed
    }

    // -----------------------------------------------------------------------
    // Serialization
    // -----------------------------------------------------------------------

    #[test]
    fn test_serialization_roundtrip() {
        let mut overrides = HashMap::new();
        overrides.insert("bash".to_string(), RiskLevel::High);
        overrides.insert("file_read".to_string(), RiskLevel::Low);
        let config = RiskConfig::custom(overrides, true, false, true);

        let json = serde_json::to_string(&config).unwrap();
        let deserialized: RiskConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(config, deserialized);
    }

    #[test]
    fn test_serialization_empty_overrides() {
        let config = RiskConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: RiskConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(config, deserialized);
    }

    #[test]
    fn test_deserialization_minimal() {
        // All fields default
        let json = r#"{"tool_overrides":{},"auto_confirm_low":true,"require_review_medium":true,"dry_run_high":true}"#;
        let config: RiskConfig = serde_json::from_str(json).unwrap();
        assert!(config.auto_confirm_low);
        assert!(config.require_review_medium);
        assert!(config.dry_run_high);
    }

    // -----------------------------------------------------------------------
    // Clone and Debug
    // -----------------------------------------------------------------------

    #[test]
    fn test_clone() {
        let mut overrides = HashMap::new();
        overrides.insert("bash".to_string(), RiskLevel::High);
        let config = RiskConfig::new(overrides);
        let cloned = config.clone();

        assert_eq!(config, cloned);
        // Verify independent
        assert_eq!(config.get_override("bash"), cloned.get_override("bash"));
    }

    #[test]
    fn test_debug() {
        let config = RiskConfig::default();
        let debug = format!("{:?}", config);
        assert!(debug.contains("auto_confirm_low"));
        assert!(debug.contains("require_review_medium"));
        assert!(debug.contains("dry_run_high"));
        assert!(debug.contains("tool_overrides"));
    }

    // -----------------------------------------------------------------------
    // Edge cases
    // -----------------------------------------------------------------------

    #[test]
    fn test_empty_tool_name() {
        let mut config = RiskConfig::default();
        config.set_override(String::new(), RiskLevel::High);
        assert!(config.has_override(""));
        assert_eq!(config.get_override(""), Some(&RiskLevel::High));
    }

    #[test]
    fn test_case_sensitive_tool_names() {
        let mut config = RiskConfig::default();
        config.set_override("Bash".to_string(), RiskLevel::Low);
        config.set_override("bash".to_string(), RiskLevel::High);

        // Different keys
        assert_eq!(config.get_override("Bash"), Some(&RiskLevel::Low));
        assert_eq!(config.get_override("bash"), Some(&RiskLevel::High));
    }

    #[test]
    fn test_override_removal_restores_default() {
        let mut config = RiskConfig::default();
        config.set_override("bash".to_string(), RiskLevel::Low);
        config.remove_override("bash");

        // After removal, classifier will use default rules (not stored in config)
        assert!(!config.has_override("bash"));
    }

    #[test]
    fn test_many_overrides() {
        let mut overrides = HashMap::new();
        for i in 0..100 {
            overrides.insert(format!("tool_{}", i), RiskLevel::Low);
        }
        let config = RiskConfig::new(overrides);
        assert_eq!(config.tool_overrides.len(), 100);
        assert!(config.has_override("tool_0"));
        assert!(config.has_override("tool_99"));
    }
}
