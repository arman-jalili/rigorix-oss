//! Implementation of `AuditBackendFactory`.
//!
//! @canonical actions/.pi/architecture/modules/audit-posting.md#backend-factory
//! Implements: AuditBackendFactory trait — creates AuditBackend instances from config
//! Issue: issue-auditbackend-trait-open-core-boundary-
//!
//! Creates the appropriate `AuditBackend` implementation based on configuration:
//! - HTTP backend when `backend_url` is configured
//! - Filesystem backend (OSS default) when `filesystem_path` is configured

use async_trait::async_trait;
use std::path::PathBuf;

use crate::audit_posting::domain::AuditPostingError;
use crate::audit_posting::infrastructure::FilesystemAuditBackendImpl;
use crate::audit_posting::infrastructure::HttpAuditBackend;
use crate::audit_posting::infrastructure::repository::AuditBackend;

use super::dto::AuditBackendConfig;
use super::factory::AuditBackendFactory;

/// Factory for creating `AuditBackend` instances from configuration.
#[derive(Debug)]
pub struct AuditBackendFactoryImpl;

impl AuditBackendFactoryImpl {
    /// Create a new factory instance.
    pub fn new() -> Self {
        Self
    }
}

impl Default for AuditBackendFactoryImpl {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AuditBackendFactory for AuditBackendFactoryImpl {
    async fn create(
        &self,
        config: AuditBackendConfig,
    ) -> Result<Box<dyn AuditBackend>, AuditPostingError> {
        // HTTP backend takes precedence if both are configured
        if let Some(url) = &config.backend_url {
            if !url.is_empty() {
                return Ok(Box::new(HttpAuditBackend::new(Some(url.clone()))));
            }
        }

        if let Some(path) = &config.filesystem_path {
            if !path.is_empty() {
                return Ok(Box::new(FilesystemAuditBackendImpl::new(PathBuf::from(
                    path,
                ))));
            }
        }

        Err(AuditPostingError::NotConfigured {
            missing_field: "backend_url or filesystem_path".to_string(),
        })
    }

    async fn create_default(&self) -> Result<Box<dyn AuditBackend>, AuditPostingError> {
        // Default to a `.rigorix/audit-records/` directory in the current working directory
        let default_path = PathBuf::from(".rigorix/audit-records");
        Ok(Box::new(FilesystemAuditBackendImpl::new(default_path)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_http_backend() {
        let factory = AuditBackendFactoryImpl::new();
        let config = AuditBackendConfig {
            backend_url: Some("https://audit.example.com".to_string()),
            filesystem_path: None,
            signing_key: None,
            key_id: None,
            max_retries: 3,
            retry_delay_secs: 1,
            queue_capacity: 100,
        };
        let backend = factory.create(config).await.unwrap();
        // Should be HttpAuditBackend — can't check type directly, but can call methods
        assert!(!backend.health_check().await.unwrap());
    }

    #[tokio::test]
    async fn test_create_filesystem_backend() {
        let factory = AuditBackendFactoryImpl::new();
        let config = AuditBackendConfig {
            backend_url: None,
            filesystem_path: Some("/tmp/test-audit".to_string()),
            signing_key: None,
            key_id: None,
            max_retries: 3,
            retry_delay_secs: 1,
            queue_capacity: 100,
        };
        let backend = factory.create(config).await.unwrap();
        assert!(backend.health_check().await.unwrap());
        // Cleanup
        std::fs::remove_dir_all("/tmp/test-audit").ok();
    }

    #[tokio::test]
    async fn test_create_http_over_filesystem() {
        let factory = AuditBackendFactoryImpl::new();
        let config = AuditBackendConfig {
            backend_url: Some("https://audit.example.com".to_string()),
            filesystem_path: Some("/tmp/test-audit".to_string()),
            signing_key: None,
            key_id: None,
            max_retries: 3,
            retry_delay_secs: 1,
            queue_capacity: 100,
        };
        let backend = factory.create(config).await.unwrap();
        // HTTP backend — health check will return false (no real server)
        assert!(!backend.health_check().await.unwrap());
    }

    #[tokio::test]
    async fn test_create_no_config() {
        let factory = AuditBackendFactoryImpl::new();
        let config = AuditBackendConfig {
            backend_url: None,
            filesystem_path: None,
            signing_key: None,
            key_id: None,
            max_retries: 3,
            retry_delay_secs: 1,
            queue_capacity: 100,
        };
        let result = factory.create(config).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            AuditPostingError::NotConfigured { .. } => {}
            other => panic!("Expected NotConfigured, got: {other}"),
        }
    }

    #[tokio::test]
    async fn test_create_default() {
        let factory = AuditBackendFactoryImpl::new();
        let backend = factory.create_default().await.unwrap();
        // Default is filesystem — should create directory and be healthy
        assert!(backend.health_check().await.unwrap());
    }
}
