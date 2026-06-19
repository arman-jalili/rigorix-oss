//! QualityGateConfig — per-task quality requirements configuration.
//!
//! @canonical .pi/architecture/modules/quality-gates.md#config
//! Implements: Contract Freeze — QualityGateConfig struct
//! Issue: #449 (quality-gates epic)
//!
//! # Contract (Frozen)
//! - Carries per-task quality gate configuration
//! - `default_required_level` sets the fallback for tasks without overrides
//! - `template_overrides` allows per-template level customization
//! - Implements `Clone`, `Debug`, `PartialEq` for testability
//! - Serialization support for TOML/JSON configuration files

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::level::QualityLevel;

/// Default quality level for serde deserialization.
fn default_quality_level() -> QualityLevel {
    QualityLevel::Package
}

/// Configuration for quality gate evaluation.
///
/// Defines the default required quality level for tasks and per-template
/// overrides. Loaded from configuration sources (e.g., `.rigorix/quality.toml`).
///
/// # Example
///
/// ```toml
/// [quality]
/// default_required_level = "package"
///
/// [quality.templates.refactor]
/// required_level = "workspace"
///
/// [quality.templates.hotfix]
/// required_level = "merge_ready"
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QualityGateConfig {
    /// Default required quality level for all tasks.
    /// Used when no per-template override exists.
    #[serde(default = "default_quality_level")]
    pub default_required_level: QualityLevel,

    /// Per-template overrides for required quality level.
    /// Keys are template names (e.g., "refactor", "hotfix", "feature").
    #[serde(default)]
    pub template_overrides: HashMap<String, QualityLevel>,
}

impl QualityGateConfig {
    /// Create a new `QualityGateConfig` with the given default level.
    pub fn new(default_required_level: QualityLevel) -> Self {
        Self {
            default_required_level,
            template_overrides: HashMap::new(),
        }
    }

    /// Get the required quality level for a given template name.
    ///
    /// Checks template overrides first, then falls back to the default.
    /// Returns `None` only if no default is set (shouldn't happen in practice).
    pub fn required_level_for_template(&self, template_name: &str) -> Option<QualityLevel> {
        self.template_overrides
            .get(template_name)
            .copied()
            .or(Some(self.default_required_level))
    }

    /// Add a template override.
    pub fn add_override(&mut self, template_name: &str, level: QualityLevel) {
        self.template_overrides
            .insert(template_name.to_string(), level);
    }

    /// Remove a template override.
    pub fn remove_override(&mut self, template_name: &str) -> Option<QualityLevel> {
        self.template_overrides.remove(template_name)
    }

    /// Returns `true` if there are any template overrides.
    pub fn has_overrides(&self) -> bool {
        !self.template_overrides.is_empty()
    }

    /// Returns the number of template overrides.
    pub fn override_count(&self) -> usize {
        self.template_overrides.len()
    }
}

impl Default for QualityGateConfig {
    fn default() -> Self {
        Self {
            default_required_level: QualityLevel::Package,
            template_overrides: HashMap::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_config() {
        let config = QualityGateConfig::new(QualityLevel::Workspace);
        assert_eq!(config.default_required_level, QualityLevel::Workspace);
        assert!(!config.has_overrides());
    }

    #[test]
    fn test_default_config() {
        let config = QualityGateConfig::default();
        assert_eq!(
            config.default_required_level,
            QualityLevel::Package
        );
    }

    #[test]
    fn test_required_level_for_template_falls_back_to_default() {
        let config = QualityGateConfig::new(QualityLevel::Workspace);
        let level = config.required_level_for_template("unknown");
        assert_eq!(level, Some(QualityLevel::Workspace));
    }

    #[test]
    fn test_add_override() {
        let mut config = QualityGateConfig::new(QualityLevel::Package);
        config.add_override("hotfix", QualityLevel::MergeReady);
        assert!(config.has_overrides());
        assert_eq!(config.override_count(), 1);

        let level = config.required_level_for_template("hotfix");
        assert_eq!(level, Some(QualityLevel::MergeReady));
    }

    #[test]
    fn test_remove_override() {
        let mut config = QualityGateConfig::new(QualityLevel::Package);
        config.add_override("hotfix", QualityLevel::MergeReady);
        let removed = config.remove_override("hotfix");
        assert_eq!(removed, Some(QualityLevel::MergeReady));
        assert!(!config.has_overrides());
    }

    #[test]
    fn test_template_override_takes_precedence() {
        let mut config = QualityGateConfig::new(QualityLevel::Package);
        config.add_override("feature", QualityLevel::Workspace);

        let level = config.required_level_for_template("feature");
        assert_eq!(level, Some(QualityLevel::Workspace));

        // Unknown template falls back to default
        let level = config.required_level_for_template("other");
        assert_eq!(level, Some(QualityLevel::Package));
    }

    #[test]
    fn test_multiple_overrides() {
        let mut config = QualityGateConfig::new(QualityLevel::TargetedTests);
        config.add_override("refactor", QualityLevel::Workspace);
        config.add_override("hotfix", QualityLevel::MergeReady);
        config.add_override("feature", QualityLevel::Package);

        assert_eq!(config.override_count(), 3);
        assert_eq!(
            config.required_level_for_template("refactor"),
            Some(QualityLevel::Workspace)
        );
        assert_eq!(
            config.required_level_for_template("hotfix"),
            Some(QualityLevel::MergeReady)
        );
        assert_eq!(
            config.required_level_for_template("feature"),
            Some(QualityLevel::Package)
        );
    }

    #[test]
    fn test_serialization_roundtrip() {
        let mut config = QualityGateConfig::new(QualityLevel::Workspace);
        config.add_override("hotfix", QualityLevel::MergeReady);

        let json = serde_json::to_string(&config).unwrap();
        let deserialized: QualityGateConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config, deserialized);
    }
}
