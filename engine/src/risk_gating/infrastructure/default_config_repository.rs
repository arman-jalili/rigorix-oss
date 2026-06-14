//! Default implementation of the RiskConfigRepository.
//!
//! @canonical .pi/architecture/modules/risk-gating.md
//! Implements: ISSUE-RISK-GATING-1 — RiskConfigRepository default implementation
//! Issue: #90
//!
//! Provides a default `InMemoryConfigRepository` that stores risk
//! configuration in memory. This is suitable for single-execution
//! scenarios where persistence across restarts is not required.
//!
//! # Thread Safety
//! - Config state is protected by `RwLock` for concurrent read/write
//! - All async methods are safe to call from multiple tasks

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::RwLock;

use crate::risk_gating::domain::{RiskConfig, RiskGatingError, RiskLevel};
use crate::risk_gating::infrastructure::repository::RiskConfigRepository;

/// In-memory implementation of `RiskConfigRepository`.
///
/// Stores configuration per execution ID. Not persisted across restarts.
///
/// # Examples
///
/// ```rust
/// use rigorix::risk_gating::infrastructure::InMemoryConfigRepository;
/// use rigorix::risk_gating::infrastructure::repository::RiskConfigRepository;
///
/// # async fn example() {
/// let repo = InMemoryConfigRepository::new();
/// let config = repo.load_config("exec-1").await.unwrap();
/// assert!(config.auto_confirm_low);
/// # }
/// ```
pub struct InMemoryConfigRepository {
    /// Stored configurations keyed by execution ID.
    configs: RwLock<HashMap<String, RiskConfig>>,

    /// Stored tool overrides keyed by tool name.
    overrides: RwLock<HashMap<String, RiskLevel>>,
}

impl InMemoryConfigRepository {
    /// Create a new empty `InMemoryConfigRepository`.
    pub fn new() -> Self {
        Self {
            configs: RwLock::new(HashMap::new()),
            overrides: RwLock::new(HashMap::new()),
        }
    }
}

impl Default for InMemoryConfigRepository {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl RiskConfigRepository for InMemoryConfigRepository {
    async fn load_config(&self, execution_id: &str) -> Result<RiskConfig, RiskGatingError> {
        let configs = self.configs.read().expect("ConfigRepository lock poisoned");
        Ok(configs
            .get(execution_id)
            .cloned()
            .unwrap_or_default())
    }

    async fn save_config(
        &self,
        execution_id: &str,
        config: &RiskConfig,
    ) -> Result<(), RiskGatingError> {
        let mut configs = self.configs.write().expect("ConfigRepository lock poisoned");
        configs.insert(execution_id.to_string(), config.clone());
        Ok(())
    }

    async fn load_tool_override(
        &self,
        tool: &str,
    ) -> Result<Option<RiskLevel>, RiskGatingError> {
        let overrides = self.overrides.read().expect("ConfigRepository lock poisoned");
        Ok(overrides.get(tool).copied())
    }

    async fn save_tool_override(
        &self,
        tool: &str,
        risk_level: &RiskLevel,
    ) -> Result<(), RiskGatingError> {
        let mut overrides = self.overrides.write().expect("ConfigRepository lock poisoned");
        overrides.insert(tool.to_string(), *risk_level);
        Ok(())
    }

    async fn remove_tool_override(&self, tool: &str) -> Result<bool, RiskGatingError> {
        let mut overrides = self.overrides.write().expect("ConfigRepository lock poisoned");
        Ok(overrides.remove(tool).is_some())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::risk_gating::domain::RiskConfig;

    #[tokio::test]
    async fn test_load_default_config() {
        let repo = InMemoryConfigRepository::new();
        let config = repo.load_config("exec-1").await.unwrap();
        assert!(config.auto_confirm_low);
        assert!(config.require_review_medium);
        assert!(config.dry_run_high);
        assert!(config.tool_overrides.is_empty());
    }

    #[tokio::test]
    async fn test_save_and_load_config() {
        let repo = InMemoryConfigRepository::new();
        let mut config = RiskConfig::default();
        config.auto_confirm_low = false;

        repo.save_config("exec-1", &config).await.unwrap();
        let loaded = repo.load_config("exec-1").await.unwrap();
        assert!(!loaded.auto_confirm_low);
    }

    #[tokio::test]
    async fn test_save_and_load_tool_override() {
        let repo = InMemoryConfigRepository::new();
        repo.save_tool_override("bash", &RiskLevel::Low).await.unwrap();

        let loaded = repo.load_tool_override("bash").await.unwrap();
        assert_eq!(loaded, Some(RiskLevel::Low));
    }

    #[tokio::test]
    async fn test_load_nonexistent_override() {
        let repo = InMemoryConfigRepository::new();
        let loaded = repo.load_tool_override("nonexistent").await.unwrap();
        assert_eq!(loaded, None);
    }

    #[tokio::test]
    async fn test_remove_tool_override() {
        let repo = InMemoryConfigRepository::new();
        repo.save_tool_override("bash", &RiskLevel::Low).await.unwrap();

        let removed = repo.remove_tool_override("bash").await.unwrap();
        assert!(removed);

        let loaded = repo.load_tool_override("bash").await.unwrap();
        assert_eq!(loaded, None);
    }

    #[tokio::test]
    async fn test_remove_nonexistent_override() {
        let repo = InMemoryConfigRepository::new();
        let removed = repo.remove_tool_override("nonexistent").await.unwrap();
        assert!(!removed);
    }

    #[tokio::test]
    async fn test_configs_isolated_by_execution() {
        let repo = InMemoryConfigRepository::new();
        let mut config_a = RiskConfig::default();
        config_a.auto_confirm_low = false;
        repo.save_config("exec-a", &config_a).await.unwrap();

        let config_b = repo.load_config("exec-b").await.unwrap();
        assert!(config_b.auto_confirm_low); // Default, not config_a's value
    }
}
