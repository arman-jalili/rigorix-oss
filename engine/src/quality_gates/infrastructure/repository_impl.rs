//! In-memory implementation of `QualityGateConfigRepository`.
//!
//! @canonical .pi/architecture/modules/quality-gates.md#config
//! Implements: QualityGateConfigRepository — in-memory config storage
//! Issue: #451, #452, #453
//!
//! Stores configuration in memory with `RwLock` for thread-safe access.

use async_trait::async_trait;
use std::sync::RwLock;

use crate::quality_gates::domain::{
    GreenContract, QualityGateConfig, QualityGateError, QualityLevel,
};

use super::repository::QualityGateConfigRepository;

/// In-memory implementation of `QualityGateConfigRepository`.
///
/// Stores quality gate configuration in memory. Thread-safe via `RwLock`.
pub struct InMemoryQualityGateRepository {
    /// The quality gate configuration.
    config: RwLock<QualityGateConfig>,
}

impl InMemoryQualityGateRepository {
    /// Create a new repository with the given configuration.
    pub fn new(config: QualityGateConfig) -> Self {
        Self {
            config: RwLock::new(config),
        }
    }

    /// Create a new repository with default configuration.
    pub fn new_default() -> Self {
        Self {
            config: RwLock::new(QualityGateConfig::default()),
        }
    }
}

impl Default for InMemoryQualityGateRepository {
    fn default() -> Self {
        Self::new_default()
    }
}

#[async_trait]
impl QualityGateConfigRepository for InMemoryQualityGateRepository {
    async fn load_config(&self) -> Result<QualityGateConfig, QualityGateError> {
        self.config
            .read()
            .map(|c| c.clone())
            .map_err(|e| QualityGateError::DependencyUnavailable {
                dependency: "InMemoryQualityGateRepository".to_string(),
                reason: format!("RwLock poisoned: {}", e),
            })
    }

    async fn store_config(&self, config: &QualityGateConfig) -> Result<(), QualityGateError> {
        let mut c = self.config.write().map_err(|e| {
            QualityGateError::DependencyUnavailable {
                dependency: "InMemoryQualityGateRepository".to_string(),
                reason: format!("RwLock poisoned: {}", e),
            }
        })?;
        *c = config.clone();
        Ok(())
    }

    async fn contract_for_template(
        &self,
        template_name: &str,
    ) -> Result<GreenContract, QualityGateError> {
        let config = self.load_config().await?;
        let level = config
            .required_level_for_template(template_name)
            .unwrap_or(config.default_required_level);
        Ok(GreenContract::new(level))
    }

    async fn default_contract(&self) -> Result<GreenContract, QualityGateError> {
        let config = self.load_config().await?;
        Ok(GreenContract::new(config.default_required_level))
    }

    async fn set_default_level(&self, level: QualityLevel) -> Result<(), QualityGateError> {
        let mut config = self.config.write().map_err(|e| {
            QualityGateError::DependencyUnavailable {
                dependency: "InMemoryQualityGateRepository".to_string(),
                reason: format!("RwLock poisoned: {}", e),
            }
        })?;
        config.default_required_level = level;
        Ok(())
    }

    async fn set_template_level(
        &self,
        template_name: &str,
        level: QualityLevel,
    ) -> Result<(), QualityGateError> {
        let mut config = self.config.write().map_err(|e| {
            QualityGateError::DependencyUnavailable {
                dependency: "InMemoryQualityGateRepository".to_string(),
                reason: format!("RwLock poisoned: {}", e),
            }
        })?;
        config.add_override(template_name, level);
        Ok(())
    }

    async fn remove_template_override(
        &self,
        template_name: &str,
    ) -> Result<Option<QualityLevel>, QualityGateError> {
        let mut config = self.config.write().map_err(|e| {
            QualityGateError::DependencyUnavailable {
                dependency: "InMemoryQualityGateRepository".to_string(),
                reason: format!("RwLock poisoned: {}", e),
            }
        })?;
        Ok(config.remove_override(template_name))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_default_contract() {
        let repo = InMemoryQualityGateRepository::new_default();
        let contract = repo.default_contract().await.unwrap();
        assert_eq!(contract.required_level, QualityLevel::Package);
    }

    #[tokio::test]
    async fn test_load_config() {
        let config = QualityGateConfig::new(QualityLevel::Workspace);
        let repo = InMemoryQualityGateRepository::new(config);
        let loaded = repo.load_config().await.unwrap();
        assert_eq!(loaded.default_required_level, QualityLevel::Workspace);
    }

    #[tokio::test]
    async fn test_store_config() {
        let repo = InMemoryQualityGateRepository::new_default();
        let new_config = QualityGateConfig::new(QualityLevel::MergeReady);
        repo.store_config(&new_config).await.unwrap();
        let loaded = repo.load_config().await.unwrap();
        assert_eq!(
            loaded.default_required_level,
            QualityLevel::MergeReady
        );
    }

    #[tokio::test]
    async fn test_set_default_level() {
        let repo = InMemoryQualityGateRepository::new_default();
        repo.set_default_level(QualityLevel::Workspace).await.unwrap();
        let contract = repo.default_contract().await.unwrap();
        assert_eq!(contract.required_level, QualityLevel::Workspace);
    }

    #[tokio::test]
    async fn test_contract_for_template() {
        let mut config = QualityGateConfig::new(QualityLevel::Package);
        config.add_override("hotfix", QualityLevel::MergeReady);
        let repo = InMemoryQualityGateRepository::new(config);

        let contract = repo
            .contract_for_template("hotfix")
            .await
            .unwrap();
        assert_eq!(contract.required_level, QualityLevel::MergeReady);

        // Unknown template falls back to default
        let contract = repo
            .contract_for_template("unknown")
            .await
            .unwrap();
        assert_eq!(contract.required_level, QualityLevel::Package);
    }

    #[tokio::test]
    async fn test_set_template_level() {
        let repo = InMemoryQualityGateRepository::new_default();
        repo.set_template_level("hotfix", QualityLevel::MergeReady)
            .await
            .unwrap();
        let contract = repo.contract_for_template("hotfix").await.unwrap();
        assert_eq!(contract.required_level, QualityLevel::MergeReady);
    }

    #[tokio::test]
    async fn test_remove_template_override() {
        let mut config = QualityGateConfig::new(QualityLevel::Package);
        config.add_override("hotfix", QualityLevel::MergeReady);
        let repo = InMemoryQualityGateRepository::new(config);

        let removed = repo
            .remove_template_override("hotfix")
            .await
            .unwrap();
        assert_eq!(removed, Some(QualityLevel::MergeReady));

        let removed = repo
            .remove_template_override("hotfix")
            .await
            .unwrap();
        assert_eq!(removed, None);
    }
}
