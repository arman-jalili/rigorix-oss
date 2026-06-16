//! Default implementation of `EnforcementPolicyRepository`.
//!
//! @canonical .pi/architecture/modules/enforcement.md#infrastructure
//! Implements: ISSUE-ENFORCEMENT-1 — Default policy repository
//! Issue: #58
//!
//! The default repository loads enforcement configuration from the global
//! configuration and provides in-memory storage for runtime policy overrides.
//! No external persistence is required — the default config is sufficient
//! for most use cases.

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::RwLock;

use crate::enforcement::domain::{EnforcementConfig, EnforcementError, ToolPolicy};

use super::repository::EnforcementPolicyRepository;

/// Default in-memory implementation of `EnforcementPolicyRepository`.
///
/// Loads configuration from the provided `EnforcementConfig` at construction
/// time. Stores runtime tool policy overrides in memory. No external
/// persistence is used.
pub struct DefaultPolicyRepository {
    /// The base enforcement configuration.
    config: RwLock<EnforcementConfig>,

    /// Runtime tool policy overrides (applied on top of base config).
    tool_overrides: RwLock<HashMap<String, ToolPolicy>>,
}

impl DefaultPolicyRepository {
    /// Create a new `DefaultPolicyRepository` from an `EnforcementConfig`.
    pub fn new(config: EnforcementConfig) -> Self {
        Self {
            config: RwLock::new(config),
            tool_overrides: RwLock::new(HashMap::new()),
        }
    }

    /// Create a new `DefaultPolicyRepository` using the Standard preset.
    #[allow(clippy::should_implement_trait)]
    pub fn default() -> Self {
        Self::new(EnforcementConfig::standard())
    }
}

#[async_trait]
impl EnforcementPolicyRepository for DefaultPolicyRepository {
    async fn load_config(
        &self,
        _execution_id: &str,
    ) -> Result<EnforcementConfig, EnforcementError> {
        let config = self
            .config
            .read()
            .map_err(|e| EnforcementError::InvalidState {
                detail: format!("Failed to read config: {}", e),
            })?;
        Ok(config.clone())
    }

    async fn save_config(
        &self,
        _execution_id: &str,
        config: &EnforcementConfig,
    ) -> Result<(), EnforcementError> {
        let mut current = self
            .config
            .write()
            .map_err(|e| EnforcementError::InvalidState {
                detail: format!("Failed to write config: {}", e),
            })?;
        *current = config.clone();
        Ok(())
    }

    async fn load_tool_policy(&self, tool: &str) -> Result<Option<ToolPolicy>, EnforcementError> {
        // Check runtime overrides first
        {
            let overrides =
                self.tool_overrides
                    .read()
                    .map_err(|e| EnforcementError::InvalidState {
                        detail: format!("Failed to read overrides: {}", e),
                    })?;
            if let Some(policy) = overrides.get(tool) {
                return Ok(Some(policy.clone()));
            }
        }

        // Fall back to base config tool policies
        let config = self
            .config
            .read()
            .map_err(|e| EnforcementError::InvalidState {
                detail: format!("Failed to read config: {}", e),
            })?;
        Ok(config.tool_policies.get(tool).cloned())
    }

    async fn save_tool_policy(
        &self,
        tool: &str,
        policy: &ToolPolicy,
    ) -> Result<(), EnforcementError> {
        let mut overrides =
            self.tool_overrides
                .write()
                .map_err(|e| EnforcementError::InvalidState {
                    detail: format!("Failed to write overrides: {}", e),
                })?;
        overrides.insert(tool.to_string(), policy.clone());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::enforcement::domain::{ResourceBudget, ToolRiskLevel};
    use std::collections::HashMap;

    fn create_test_config() -> EnforcementConfig {
        let mut budgets = HashMap::new();
        budgets.insert(
            "tokens".to_string(),
            ResourceBudget {
                resource: "tokens".to_string(),
                soft_warning_threshold: 0.8,
                hard_limit: 100_000,
                current_usage: 0,
            },
        );

        let mut tool_policies = HashMap::new();
        tool_policies.insert(
            "bash".to_string(),
            ToolPolicy {
                allowed: true,
                risk_level: ToolRiskLevel::High,
                requires_confirmation: true,
                dry_run: false,
                max_calls: Some(100),
                budget_key: Some("tool_calls".to_string()),
            },
        );

        EnforcementConfig {
            budgets,
            tool_policies,
            ..EnforcementConfig::default()
        }
    }

    #[tokio::test]
    async fn test_load_default_config() {
        let repo = DefaultPolicyRepository::new(create_test_config());
        let config = repo.load_config("test-execution").await.unwrap();
        assert!(config.budgets.contains_key("tokens"));
        assert!(config.tool_policies.contains_key("bash"));
    }

    #[tokio::test]
    async fn test_save_and_load_config() {
        let repo = DefaultPolicyRepository::new(EnforcementConfig::standard());
        let strict_config = EnforcementConfig::strict();
        repo.save_config("test", &strict_config).await.unwrap();
        let loaded = repo.load_config("test").await.unwrap();
        assert_eq!(
            loaded.preset,
            crate::enforcement::domain::EnforcementPresetProfile::Strict
        );
    }

    #[tokio::test]
    async fn test_load_tool_policy_from_config() {
        let config = create_test_config();
        let repo = DefaultPolicyRepository::new(config);
        let policy = repo.load_tool_policy("bash").await.unwrap();
        assert!(policy.is_some());
        assert!(policy.unwrap().allowed);
    }

    #[tokio::test]
    async fn test_load_tool_policy_not_found() {
        let repo = DefaultPolicyRepository::new(EnforcementConfig::standard());
        let policy = repo.load_tool_policy("nonexistent_tool").await.unwrap();
        assert!(policy.is_none());
    }

    #[tokio::test]
    async fn test_save_and_load_tool_policy_override() {
        let repo = DefaultPolicyRepository::new(create_test_config());

        // Override bash to be blocked
        let blocked = ToolPolicy {
            allowed: false,
            ..ToolPolicy::default()
        };
        repo.save_tool_policy("bash", &blocked).await.unwrap();

        // Should return the override
        let policy = repo.load_tool_policy("bash").await.unwrap().unwrap();
        assert!(!policy.allowed);
    }

    #[tokio::test]
    async fn test_tool_policy_override_precedes_config() {
        let config = create_test_config();
        let repo = DefaultPolicyRepository::new(config);

        // Config says bash is allowed
        let policy = repo.load_tool_policy("bash").await.unwrap().unwrap();
        assert!(policy.allowed);

        // Override to blocked
        let blocked = ToolPolicy {
            allowed: false,
            ..ToolPolicy::default()
        };
        repo.save_tool_policy("bash", &blocked).await.unwrap();

        // Now returns blocked (override wins)
        let policy = repo.load_tool_policy("bash").await.unwrap().unwrap();
        assert!(!policy.allowed);
    }

    #[tokio::test]
    async fn test_multiple_tool_overrides() {
        let repo = DefaultPolicyRepository::new(EnforcementConfig::standard());

        let bash_policy = ToolPolicy {
            allowed: false,
            ..ToolPolicy::default()
        };
        let write_policy = ToolPolicy {
            dry_run: true,
            ..ToolPolicy::default()
        };

        repo.save_tool_policy("bash", &bash_policy).await.unwrap();
        repo.save_tool_policy("write", &write_policy).await.unwrap();

        assert!(
            !repo
                .load_tool_policy("bash")
                .await
                .unwrap()
                .unwrap()
                .allowed
        );
        assert!(
            repo.load_tool_policy("write")
                .await
                .unwrap()
                .unwrap()
                .dry_run
        );
    }

    #[tokio::test]
    async fn test_default_repository() {
        let repo = DefaultPolicyRepository::default();
        let config = repo.load_config("test").await.unwrap();
        assert_eq!(
            config.preset,
            crate::enforcement::domain::EnforcementPresetProfile::Standard
        );
    }
}
