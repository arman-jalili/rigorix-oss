//! Implementation of `ConfigWriteRepository`.
//!
//! @canonical .pi/architecture/modules/configuration.md
//! Implements: GAP-A-16 — ConfigWriteRepository impl
//!
//! In-memory write cache for the resolved `ConfigDto` (filesystem/env
//! reads live on `ConfigRepository`).

use async_trait::async_trait;
use std::sync::RwLock;

use crate::configuration::application::dto::ConfigDto;
use crate::configuration::domain::error::ConfigurationError;
use crate::configuration::infrastructure::repository::ConfigWriteRepository;

/// Filesystem + env backed `ConfigWriteRepository`.
pub struct ConfigWriteRepositoryImpl {
    /// Cached resolved config (in-memory).
    cache: RwLock<Option<ConfigDto>>,
}

impl ConfigWriteRepositoryImpl {
    /// Create a new implementation with an empty cache.
    pub fn new() -> Self {
        Self {
            cache: RwLock::new(None),
        }
    }
}

impl Default for ConfigWriteRepositoryImpl {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ConfigWriteRepository for ConfigWriteRepositoryImpl {
    async fn write_cached(&self, config: &ConfigDto) -> Result<(), ConfigurationError> {
        *self.cache.write().unwrap_or_else(|e| e.into_inner()) = Some(config.clone());
        Ok(())
    }

    async fn read_cached(&self) -> Result<Option<ConfigDto>, ConfigurationError> {
        Ok(self.cache.read().unwrap_or_else(|e| e.into_inner()).clone())
    }

    async fn invalidate_cache(&self) -> Result<(), ConfigurationError> {
        *self.cache.write().unwrap_or_else(|e| e.into_inner()) = None;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_cache_round_trip() {
        let repo = ConfigWriteRepositoryImpl::new();
        let config = ConfigDto {
            orchestrator: Default::default(),
            logging: Default::default(),
            tools: crate::configuration::application::dto::ToolsConfigDto {
                tool_overrides: std::collections::HashMap::new(),
                auto_confirm_low: true,
                require_review_medium: true,
                dry_run_high: true,
            },
            enforcement: Default::default(),
            audit: Default::default(),
            llm: crate::configuration::application::dto::LlmConfigDto {
                provider: "anthropic".to_string(),
                model: "test-model".to_string(),
                base_url: None,
                max_tokens: 1024,
                temperature: 0.2,
                api_key: crate::configuration::domain::Secret::from("test-key"),
            },
            enforcement_backend_url: None,
            enforcement_backend_key: None,
            audit_backend_url: None,
            audit_backend_key: None,
        };
        repo.write_cached(&config).await.unwrap();
        let loaded = repo.read_cached().await.unwrap();
        assert!(loaded.is_some());
        repo.invalidate_cache().await.unwrap();
        assert!(repo.read_cached().await.unwrap().is_none());
    }
}
