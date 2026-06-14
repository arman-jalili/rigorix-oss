//! Default RiskClassifier implementation — built-in tool-to-risk mapping.
//!
//! @canonical .pi/architecture/modules/risk-gating.md#classifier
//! Implements: ISSUE-RISK-GATING-1 — RiskClassifier default implementation
//! Issue: #90
//!
//! Provides a concrete `DefaultClassifier` with built-in classification rules
//! for known tools. Supports configuration-driven overrides via `RiskConfig`.
//!
//! # Classification Rules
//!
//! | Tool Pattern                          | Risk Level |
//! |---------------------------------------|------------|
//! | `file_read`, `lsp_query`, `git_read`  | Low        |
//! | `file_write`, `file_append`,          | Medium     |
//! | `file_patch`, `git_stage`             |            |
//! | `run_command`, `git_commit`,          | High       |
//! | `git_push`                            |            |
//!
//! Unknown tools default to `Medium` (safe default: require confirmation).
//!
//! # Thread Safety
//! - The classifier is immutable after construction (overrides via cloned config)
//! - All methods are `&self` — safe for concurrent access
//! - Send + Sync for trait object usage

use std::collections::HashMap;

use crate::risk_gating::domain::risk_classifier::{ClassificationResult, RiskClassifier};
use crate::risk_gating::domain::risk_level::RiskLevel;
use crate::risk_gating::domain::RiskConfig;

/// A classifier that uses a combination of configured overrides and
/// built-in default rules to determine tool risk levels.
///
/// Overrides from `RiskConfig.tool_overrides` take precedence over
/// built-in default rules. Unknown tools (no override and no default
/// rule) are classified as `Medium` as a safe default.
///
/// # Examples
///
/// ```rust
/// use std::collections::HashMap;
/// use rigorix::risk_gating::domain::{
///     RiskConfig, RiskLevel, RiskClassifier,
///     default_classifier::DefaultClassifier,
/// };
///
/// let config = RiskConfig::default();
/// let classifier = DefaultClassifier::new(config);
///
/// let result = classifier.classify("file_read", None);
/// assert_eq!(result.risk_level, RiskLevel::Low);
/// ```
pub struct DefaultClassifier {
    /// The risk configuration with tool overrides and gating flags.
    config: RiskConfig,

    /// Default rule mapping: tool_name → RiskLevel.
    /// Initialized at construction. Immutable.
    default_rules: HashMap<&'static str, RiskLevel>,
}

impl DefaultClassifier {
    /// Create a new `DefaultClassifier` with the given configuration.
    pub fn new(config: RiskConfig) -> Self {
        let mut default_rules = HashMap::new();

        // -- Low risk (read-only, no side effects) --
        default_rules.insert("file_read", RiskLevel::Low);
        default_rules.insert("read", RiskLevel::Low);
        default_rules.insert("lsp_query", RiskLevel::Low);
        default_rules.insert("git_read", RiskLevel::Low);
        default_rules.insert("git_diff", RiskLevel::Low);
        default_rules.insert("git_log", RiskLevel::Low);
        default_rules.insert("git_status", RiskLevel::Low);
        default_rules.insert("glob", RiskLevel::Low);
        default_rules.insert("grep", RiskLevel::Low);
        default_rules.insert("list_files", RiskLevel::Low);
        default_rules.insert("search_files", RiskLevel::Low);

        // -- Medium risk (modifies local state, requires confirmation) --
        default_rules.insert("file_write", RiskLevel::Medium);
        default_rules.insert("write", RiskLevel::Medium);
        default_rules.insert("file_append", RiskLevel::Medium);
        default_rules.insert("file_patch", RiskLevel::Medium);
        default_rules.insert("edit", RiskLevel::Medium);
        default_rules.insert("git_stage", RiskLevel::Medium);
        default_rules.insert("git_add", RiskLevel::Medium);
        default_rules.insert("create_file", RiskLevel::Medium);

        // -- High risk (external execution, irreversible) --
        default_rules.insert("run_command", RiskLevel::High);
        default_rules.insert("bash", RiskLevel::High);
        default_rules.insert("git_commit", RiskLevel::High);
        default_rules.insert("git_push", RiskLevel::High);
        default_rules.insert("git_reset", RiskLevel::High);
        default_rules.insert("delete_file", RiskLevel::High);
        default_rules.insert("remove", RiskLevel::High);

        Self {
            config,
            default_rules,
        }
    }

    /// Get a reference to the underlying configuration.
    pub fn config(&self) -> &RiskConfig {
        &self.config
    }

    /// Replace the configuration (e.g., after a reload).
    pub fn set_config(&mut self, config: RiskConfig) {
        self.config = config;
    }

    /// Check if a tool name matches a known default rule.
    /// Returns the matched rule's key (for display) and the risk level.
    fn match_default_rule(&self, tool_name: &str) -> Option<(&'static str, RiskLevel)> {
        let lower = tool_name.to_lowercase();

        for (&pattern, &level) in &self.default_rules {
            if tool_name == pattern || lower == pattern || lower.starts_with(pattern) {
                return Some((pattern, level));
            }
        }

        None
    }
}

impl RiskClassifier for DefaultClassifier {
    fn classify(&self, tool_name: &str, _parameters: Option<&serde_json::Value>) -> ClassificationResult {
        // 1. Check for configured override first (highest priority)
        if let Some(override_level) = self.config.get_override(tool_name) {
            return ClassificationResult {
                risk_level: *override_level,
                reason: format!("Override from config: {} → {}", tool_name, serde_json::to_string(override_level).unwrap_or_default()),
                from_override: true,
            };
        }

        // 2. Check for lowercase override
        let lower = tool_name.to_lowercase();
        if let Some(override_level) = self.config.get_override(&lower) {
            return ClassificationResult {
                risk_level: *override_level,
                reason: format!("Override from config: {} → {}", tool_name, serde_json::to_string(override_level).unwrap_or_default()),
                from_override: true,
            };
        }

        // 3. Try default rules
        if let Some((pattern, level)) = self.match_default_rule(tool_name) {
            return ClassificationResult {
                risk_level: level,
                reason: format!("Default rule: {} → {:?}", pattern, level),
                from_override: false,
            };
        }

        // 4. Unknown tool: default to Medium (safe default: require confirmation)
        ClassificationResult {
            risk_level: RiskLevel::Medium,
            reason: format!("Unknown tool '{}', defaulting to Medium (safe default)", tool_name),
            from_override: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn create_classifier() -> DefaultClassifier {
        DefaultClassifier::new(RiskConfig::default())
    }

    #[test]
    fn test_classify_file_read_low() {
        let classifier = create_classifier();
        let result = classifier.classify("file_read", None);
        assert_eq!(result.risk_level, RiskLevel::Low);
        assert!(!result.from_override);
        assert!(result.reason.contains("file_read"));
    }

    #[test]
    fn test_classify_read_low() {
        let classifier = create_classifier();
        let result = classifier.classify("read", None);
        assert_eq!(result.risk_level, RiskLevel::Low);
    }

    #[test]
    fn test_classify_file_write_medium() {
        let classifier = create_classifier();
        let result = classifier.classify("file_write", None);
        assert_eq!(result.risk_level, RiskLevel::Medium);
        assert!(!result.from_override);
    }

    #[test]
    fn test_classify_write_medium() {
        let classifier = create_classifier();
        let result = classifier.classify("write", None);
        assert_eq!(result.risk_level, RiskLevel::Medium);
    }

    #[test]
    fn test_classify_run_command_high() {
        let classifier = create_classifier();
        let result = classifier.classify("run_command", None);
        assert_eq!(result.risk_level, RiskLevel::High);
    }

    #[test]
    fn test_classify_bash_high() {
        let classifier = create_classifier();
        let result = classifier.classify("bash", None);
        assert_eq!(result.risk_level, RiskLevel::High);
    }

    #[test]
    fn test_classify_git_commit_high() {
        let classifier = create_classifier();
        let result = classifier.classify("git_commit", None);
        assert_eq!(result.risk_level, RiskLevel::High);
    }

    #[test]
    fn test_classify_git_push_high() {
        let classifier = create_classifier();
        let result = classifier.classify("git_push", None);
        assert_eq!(result.risk_level, RiskLevel::High);
    }

    #[test]
    fn test_classify_unknown_defaults_to_medium() {
        let classifier = create_classifier();
        let result = classifier.classify("unknown_tool_xyz", None);
        assert_eq!(result.risk_level, RiskLevel::Medium);
        assert!(result.reason.contains("Unknown tool"));
    }

    #[test]
    fn test_classify_override_takes_precedence() {
        let mut overrides = HashMap::new();
        overrides.insert("file_read".to_string(), RiskLevel::High);
        let config = RiskConfig::new(overrides);
        let classifier = DefaultClassifier::new(config);

        let result = classifier.classify("file_read", None);
        assert_eq!(result.risk_level, RiskLevel::High);
        assert!(result.from_override);
        assert!(result.reason.contains("Override"));
    }

    #[test]
    fn test_classify_deterministic() {
        let classifier = create_classifier();
        let result1 = classifier.classify("file_write", None);
        let result2 = classifier.classify("file_write", None);
        assert_eq!(result1.risk_level, result2.risk_level);
        assert_eq!(result1.reason, result2.reason);
    }

    #[test]
    fn test_risk_level_convenience() {
        let classifier = create_classifier();
        let level = classifier.risk_level("file_read", None);
        assert_eq!(level, RiskLevel::Low);
    }

    #[test]
    fn test_prefix_matching() {
        let classifier = create_classifier();
        // Similar tools should still match the file_read rule
        let result = classifier.classify("file_read_custom", None);
        // This should match the "file_read" prefix
        assert_eq!(result.risk_level, RiskLevel::Low);
    }

    #[test]
    fn test_case_insensitive_override() {
        let mut overrides = HashMap::new();
        overrides.insert("run_command".to_string(), RiskLevel::Low);
        let config = RiskConfig::new(overrides);
        let classifier = DefaultClassifier::new(config);

        let result = classifier.classify("RUN_COMMAND", None);
        assert_eq!(result.risk_level, RiskLevel::Low);
        assert!(result.from_override);
    }
}
