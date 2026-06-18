//! Implementation of enforcement configuration building and validation.
//!
//! @canonical .pi/architecture/modules/enforcement.md#application
//! Implements: ISSUE-ENFORCEMENT-1 — EnforcementConfig preset builders and validation
//! Issue: #58
//!
//! Provides concrete implementations for building `EnforcementConfig` instances
//! from `EnforcementPresetProfile` selections, merging user overrides, and
//! validating configurations against safety hard caps.
//!
//! # Contract (Frozen)
//! - All preset profiles produce deterministic, safe configurations
//! - Validation is always performed before returning a configuration
//! - Warnings are non-blocking; hard cap violations are errors

use crate::enforcement::domain::{
    ConfigValidationError, EnforcementConfig, EnforcementPresetProfile, SafetyCaps,
};

/// Build and validate enforcement configurations from preset profiles.
///
/// Provides methods to:
/// - Build configs from preset profiles (Standard, Strict, Maximum)
/// - Apply user overrides to budgets and tool policies
/// - Validate configs against safety hard caps
/// - Merge multiple config sources
pub struct ConfigBuilder;

impl ConfigBuilder {
    /// Build an `EnforcementConfig` from a preset profile.
    ///
    /// Returns the config immediately. If validation is needed,
    /// call `validate()` separately.
    pub fn build_from_preset(preset: &EnforcementPresetProfile) -> EnforcementConfig {
        EnforcementConfig::from_preset(preset)
    }

    /// Build and validate a config from a preset profile.
    ///
    /// Returns the config if it passes safety cap validation, or
    /// a list of validation errors if any caps are exceeded.
    pub fn build_from_preset_with_validation(
        preset: &EnforcementPresetProfile,
        safety_caps: Option<&SafetyCaps>,
    ) -> Result<EnforcementConfig, Vec<ConfigValidationError>> {
        let config = Self::build_from_preset(preset);
        let default_caps = SafetyCaps::default();
        let caps = safety_caps.unwrap_or(&default_caps);
        let errors = config.validate(caps);
        if errors.is_empty() {
            Ok(config)
        } else {
            Err(errors)
        }
    }

    /// Merge a user-provided `EnforcementConfig` with the preset defaults.
    ///
    /// User budgets and policies take precedence over preset defaults.
    /// Execution limits from `user_config` override preset defaults if set
    /// (non-default), otherwise preset values are kept.
    pub fn merge_with_preset(
        preset: &EnforcementPresetProfile,
        user_config: EnforcementConfig,
    ) -> EnforcementConfig {
        let mut config = Self::build_from_preset(preset);

        // Merge budgets: user budgets override preset budgets
        for (name, budget) in user_config.budgets {
            config.budgets.insert(name, budget);
        }

        // Merge tool policies: user policies override preset policies
        for (tool, policy) in user_config.tool_policies {
            config.tool_policies.insert(tool, policy);
        }

        // Apply user execution limits if they differ from defaults
        let user_defaults = EnforcementConfig::default();
        if user_config.execution_limits != user_defaults.execution_limits {
            config.execution_limits = user_config.execution_limits;
        }

        // Apply user default tool policy if it differs from defaults
        if user_config.default_tool_policy != user_defaults.default_tool_policy {
            config.default_tool_policy = user_config.default_tool_policy;
        }

        config
    }

    /// Validate an `EnforcementConfig` against safety caps.
    ///
    /// Returns `Ok(())` if all values are within bounds, or a list
    /// of validation errors.
    pub fn validate(
        config: &EnforcementConfig,
        safety_caps: Option<&SafetyCaps>,
    ) -> Result<(), Vec<ConfigValidationError>> {
        let default_caps = SafetyCaps::default();
        let caps = safety_caps.unwrap_or(&default_caps);
        let errors = config.validate(caps);
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Check if a specific tool policy is valid.
    ///
    /// Validates that:
    /// - `max_calls` is not 0 (if set)
    /// - The budget_key references an existing budget (if set)
    pub fn validate_tool_policy(
        policy: &crate::enforcement::domain::ToolPolicy,
        config: &EnforcementConfig,
    ) -> Result<(), Vec<ConfigValidationError>> {
        let mut errors = Vec::new();

        if let Some(max_calls) = policy.max_calls
            && max_calls == 0
        {
            errors.push(ConfigValidationError {
                field: "tool_policy.max_calls".to_string(),
                message: "max_calls must be greater than 0 if set".to_string(),
                value: Some("0".to_string()),
            });
        }

        if let Some(budget_key) = &policy.budget_key
            && !config.budgets.contains_key(budget_key)
        {
            errors.push(ConfigValidationError {
                field: "tool_policy.budget_key".to_string(),
                message: format!(
                    "budget_key '{}' does not reference an existing budget",
                    budget_key
                ),
                value: Some(budget_key.clone()),
            });
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::enforcement::domain::{
        EnforcementPresetProfile, SafetyCaps, ToolPolicy, ToolRiskLevel,
    };

    // -----------------------------------------------------------------------
    // Preset Building Tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_build_standard_preset() {
        let config = ConfigBuilder::build_from_preset(&EnforcementPresetProfile::Standard);
        assert_eq!(config.preset, EnforcementPresetProfile::Standard);
        assert!(config.budgets.contains_key("tokens"));
        assert!(config.budgets.contains_key("tool_calls"));
        assert!(config.budgets.contains_key("execution_time_ms"));
        assert!(config.tool_policies.contains_key("bash"));
        assert!(config.tool_policies.contains_key("write"));
        assert!(config.tool_policies.contains_key("read"));
        assert_eq!(config.execution_limits.max_tool_calls, 500);
        assert_eq!(config.execution_limits.max_tokens, 100_000);
    }

    #[test]
    fn test_build_strict_preset() {
        let config = ConfigBuilder::build_from_preset(&EnforcementPresetProfile::Strict);
        assert_eq!(config.preset, EnforcementPresetProfile::Strict);
        assert_eq!(config.budgets.get("tokens").unwrap().hard_limit, 50_000);
        assert_eq!(config.budgets.get("tool_calls").unwrap().hard_limit, 200);
        assert_eq!(config.execution_limits.max_tool_calls, 200);
        assert_eq!(config.execution_limits.max_concurrent_tools, 5);
        // Strict requires confirmation for writes
        assert!(
            config
                .tool_policies
                .get("write")
                .unwrap()
                .requires_confirmation
        );
        assert!(
            config
                .tool_policies
                .get("bash")
                .unwrap()
                .requires_confirmation
        );
    }

    #[test]
    fn test_build_maximum_preset() {
        let config = ConfigBuilder::build_from_preset(&EnforcementPresetProfile::Maximum);
        assert_eq!(config.preset, EnforcementPresetProfile::Maximum);
        assert_eq!(config.budgets.get("tokens").unwrap().hard_limit, 20_000);
        assert_eq!(config.budgets.get("tool_calls").unwrap().hard_limit, 50);
        assert_eq!(config.execution_limits.max_tool_calls, 50);
        assert_eq!(config.execution_limits.max_concurrent_tools, 2);
        // Maximum blocks bash
        assert!(!config.tool_policies.get("bash").unwrap().allowed);
        // Maximum sets dry_run on write
        assert!(config.tool_policies.get("write").unwrap().dry_run);
    }

    #[test]
    fn test_from_preset_standard() {
        let config = EnforcementConfig::from_preset(&EnforcementPresetProfile::Standard);
        assert_eq!(config.preset, EnforcementPresetProfile::Standard);
    }

    #[test]
    fn test_from_preset_strict() {
        let config = EnforcementConfig::from_preset(&EnforcementPresetProfile::Strict);
        assert_eq!(config.preset, EnforcementPresetProfile::Strict);
    }

    #[test]
    fn test_from_preset_maximum() {
        let config = EnforcementConfig::from_preset(&EnforcementPresetProfile::Maximum);
        assert_eq!(config.preset, EnforcementPresetProfile::Maximum);
    }

    // -----------------------------------------------------------------------
    // Builder Method Tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_with_budget_override() {
        let mut config = EnforcementConfig::standard();
        let original = config.budgets.get("tokens").unwrap().hard_limit;
        assert_eq!(original, 100_000);

        let override_budget = crate::enforcement::domain::ResourceBudget {
            resource: "tokens".to_string(),
            soft_warning_threshold: 0.9,
            hard_limit: 200_000,
            current_usage: 0,
        };
        config = config.with_budget(override_budget);
        assert_eq!(config.budgets.get("tokens").unwrap().hard_limit, 200_000);
    }

    #[test]
    fn test_with_tool_policy_override() {
        let mut config = EnforcementConfig::standard();
        assert!(config.tool_policies.get("bash").unwrap().allowed);

        let blocked_bash = ToolPolicy {
            allowed: false,
            risk_level: ToolRiskLevel::Critical,
            requires_confirmation: true,
            dry_run: false,
            max_calls: Some(0),
            budget_key: None,
        };
        config = config.with_tool_policy("bash", blocked_bash);
        assert!(!config.tool_policies.get("bash").unwrap().allowed);
    }

    #[test]
    fn test_with_execution_limits() {
        let mut config = EnforcementConfig::standard();
        let tight_limits = crate::enforcement::domain::ExecutionLimits {
            max_tool_calls: 10,
            max_execution_time_secs: 60,
            max_tokens: 5_000,
            max_retries_per_node: 1,
            max_concurrent_tools: 1,
        };
        config = config.with_execution_limits(tight_limits);
        assert_eq!(config.execution_limits.max_tool_calls, 10);
        assert_eq!(config.execution_limits.max_concurrent_tools, 1);
    }

    #[test]
    fn test_with_default_tool_policy() {
        let mut config = EnforcementConfig::standard();
        let restrictive_default = ToolPolicy {
            allowed: true,
            risk_level: ToolRiskLevel::High,
            requires_confirmation: true,
            dry_run: true,
            max_calls: Some(5),
            budget_key: None,
        };
        config = config.with_default_tool_policy(restrictive_default);
        assert!(config.default_tool_policy.requires_confirmation);
        assert!(config.default_tool_policy.dry_run);
    }

    // -----------------------------------------------------------------------
    // Validation Tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_standard_config_valid_against_default_caps() {
        let config = EnforcementConfig::standard();
        let caps = SafetyCaps::default();
        assert!(config.is_valid(&caps));
    }

    #[test]
    fn test_strict_config_valid_against_default_caps() {
        let config = EnforcementConfig::strict();
        let caps = SafetyCaps::default();
        assert!(config.is_valid(&caps));
    }

    #[test]
    fn test_maximum_config_valid_against_default_caps() {
        let config = EnforcementConfig::maximum();
        let caps = SafetyCaps::default();
        assert!(config.is_valid(&caps));
    }

    #[test]
    fn test_validation_detects_exceeded_tool_call_cap() {
        let mut config = EnforcementConfig::standard();
        config.execution_limits.max_tool_calls = 999_999;
        let caps = SafetyCaps {
            max_parallel_tasks_cap: 10,
            max_tool_calls_cap: 1000,
            max_retries_cap: 5,
            max_timeout_secs_cap: 3600,
            max_tokens_cap: 200_000,
            max_concurrent_tools_cap: 20,
            max_budget_cap: 1_000_000,
        };
        let errors = config.validate(&caps);
        assert!(errors.iter().any(|e| e.field.contains("max_tool_calls")));
    }

    #[test]
    fn test_validation_detects_exceeded_timeout_cap() {
        let mut config = EnforcementConfig::standard();
        config.execution_limits.max_execution_time_secs = 999_999;
        let caps = SafetyCaps {
            max_timeout_secs_cap: 3600,
            ..SafetyCaps::default()
        };
        let errors = config.validate(&caps);
        assert!(
            errors
                .iter()
                .any(|e| e.field.contains("max_execution_time_secs"))
        );
    }

    #[test]
    fn test_validation_detects_exceeded_budget_cap() {
        let mut config = EnforcementConfig::standard();
        config.budgets.get_mut("tokens").unwrap().hard_limit = 99_999_999;
        let caps = SafetyCaps {
            max_budget_cap: 1_000_000,
            ..SafetyCaps::default()
        };
        let errors = config.validate(&caps);
        assert!(errors.iter().any(|e| e.field.contains("tokens")));
    }

    #[test]
    fn test_validation_detects_invalid_warning_threshold() {
        let mut config = EnforcementConfig::standard();
        config
            .budgets
            .get_mut("tokens")
            .unwrap()
            .soft_warning_threshold = 1.5;
        let caps = SafetyCaps::default();
        let errors = config.validate(&caps);
        assert!(
            errors
                .iter()
                .any(|e| e.field.contains("soft_warning_threshold"))
        );
    }

    #[test]
    fn test_validation_passes_for_strict_config_with_tight_caps() {
        let config = EnforcementConfig::strict();
        // Caps that exactly match the strict preset values
        let caps = SafetyCaps {
            max_parallel_tasks_cap: 10,
            max_tool_calls_cap: 200,
            max_retries_cap: 5,
            max_timeout_secs_cap: 1800,
            max_tokens_cap: 100_000,
            max_concurrent_tools_cap: 5,
            max_budget_cap: 1_000_000, // Must cover execution_time_ms budget (600_000)
        };
        assert!(config.is_valid(&caps));
    }

    #[test]
    fn test_config_builder_validate_tool_policy_invalid_max_calls() {
        let config = EnforcementConfig::standard();
        let bad_policy = ToolPolicy {
            allowed: true,
            risk_level: ToolRiskLevel::Low,
            requires_confirmation: false,
            dry_run: false,
            max_calls: Some(0),
            budget_key: None,
        };
        let result = ConfigBuilder::validate_tool_policy(&bad_policy, &config);
        assert!(result.is_err());
    }

    #[test]
    fn test_config_builder_validate_tool_policy_invalid_budget_key() {
        let config = EnforcementConfig::standard();
        let bad_policy = ToolPolicy {
            allowed: true,
            risk_level: ToolRiskLevel::Low,
            requires_confirmation: false,
            dry_run: false,
            max_calls: None,
            budget_key: Some("nonexistent_budget".to_string()),
        };
        let result = ConfigBuilder::validate_tool_policy(&bad_policy, &config);
        assert!(result.is_err());
    }

    #[test]
    fn test_config_builder_validate_tool_policy_valid() {
        let config = EnforcementConfig::standard();
        let good_policy = ToolPolicy {
            allowed: true,
            risk_level: ToolRiskLevel::Low,
            requires_confirmation: false,
            dry_run: false,
            max_calls: Some(10),
            budget_key: Some("tokens".to_string()),
        };
        let result = ConfigBuilder::validate_tool_policy(&good_policy, &config);
        assert!(result.is_ok());
    }

    // -----------------------------------------------------------------------
    // Merge Tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_merge_with_preset_overrides_budgets() {
        let user_config = EnforcementConfig {
            budgets: {
                let mut b = std::collections::HashMap::new();
                b.insert(
                    "custom_resource".to_string(),
                    crate::enforcement::domain::ResourceBudget {
                        resource: "custom_resource".to_string(),
                        soft_warning_threshold: 0.5,
                        hard_limit: 100,
                        current_usage: 0,
                    },
                );
                b
            },
            ..EnforcementConfig::default()
        };

        let merged =
            ConfigBuilder::merge_with_preset(&EnforcementPresetProfile::Standard, user_config);
        assert!(merged.budgets.contains_key("custom_resource"));
        // Original preset budgets should still exist
        assert!(merged.budgets.contains_key("tokens"));
        assert!(merged.budgets.contains_key("tool_calls"));
    }

    #[test]
    fn test_merge_with_preset_overrides_tool_policies() {
        let mut user_config = EnforcementConfig::default();
        user_config.tool_policies.insert(
            "bash".to_string(),
            ToolPolicy {
                allowed: false,
                ..ToolPolicy::default()
            },
        );

        let merged =
            ConfigBuilder::merge_with_preset(&EnforcementPresetProfile::Standard, user_config);
        // User's block policy should win
        assert!(!merged.tool_policies.get("bash").unwrap().allowed);
    }

    // -----------------------------------------------------------------------
    // Builder with Validation Tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_build_from_preset_with_validation_passes() {
        let result = ConfigBuilder::build_from_preset_with_validation(
            &EnforcementPresetProfile::Standard,
            None,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_build_from_preset_with_validation_fails_with_tight_caps() {
        let tight_caps = SafetyCaps {
            max_tool_calls_cap: 10,
            ..SafetyCaps::default()
        };
        let result = ConfigBuilder::build_from_preset_with_validation(
            &EnforcementPresetProfile::Standard,
            Some(&tight_caps),
        );
        // Standard preset has max_tool_calls=500 which exceeds cap of 10
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors.iter().any(|e| e.field.contains("max_tool_calls")));
    }

    // -----------------------------------------------------------------------
    // Default Implementation Tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_default_config_is_standard() {
        let config = EnforcementConfig::default();
        assert_eq!(config.preset, EnforcementPresetProfile::Standard);
        assert_eq!(config.budgets.len(), 3);
    }

    #[test]
    fn test_default_safety_caps() {
        let caps = SafetyCaps::default();
        assert_eq!(caps.max_parallel_tasks_cap, 10);
        assert_eq!(caps.max_retries_cap, 5);
        assert_eq!(caps.max_timeout_secs_cap, 3600);
        assert_eq!(caps.max_tokens_cap, 200_000);
        assert_eq!(caps.max_concurrent_tools_cap, 20);
        assert_eq!(caps.max_budget_cap, 1_000_000);
    }

    // -----------------------------------------------------------------------
    // Serde Round-Trip Tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_config_serde_roundtrip() {
        let config = EnforcementConfig::standard();
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: EnforcementConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, config);
    }

    #[test]
    fn test_safety_caps_serde_roundtrip() {
        let caps = SafetyCaps::default();
        let json = serde_json::to_string(&caps).unwrap();
        let deserialized: SafetyCaps = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, caps);
    }
}
