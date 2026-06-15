//! Implementation of `SecretFactory`.
//!
//! @canonical .pi/architecture/modules/configuration.md#secret
//! Implements: SecretFactory trait — loads secrets from env or strings
//! Issue: #4

use async_trait::async_trait;

use crate::configuration::application::factory::SecretFactory;
use crate::configuration::domain::Secret;

/// Implementation of `SecretFactory` that reads from environment variables
/// or wraps string values.
pub struct SecretFactoryImpl;

impl SecretFactoryImpl {
    pub fn new() -> Self {
        Self
    }
}

impl Default for SecretFactoryImpl {
    #[tracing::instrument(skip_all)]
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SecretFactory for SecretFactoryImpl {
    #[tracing::instrument(skip_all)]
    async fn load_from_env(&self, env_var: &str, fallback: Option<String>) -> Option<Secret> {
        match std::env::var(env_var) {
            Ok(value) => {
                if value.is_empty() {
                    fallback.map(Secret::new)
                } else {
                    Some(Secret::new(value))
                }
            }
            Err(_) => fallback.map(Secret::new),
        }
    }

    #[tracing::instrument(skip_all)]
    fn create_from_value(&self, value: &str) -> Secret {
        Secret::new(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_load_from_env_found() {
        let factory = SecretFactoryImpl::new();
        // PATH is always set
        let result = factory.load_from_env("PATH", None).await;
        assert!(result.is_some());
        assert!(!result.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_load_from_env_not_found_no_fallback() {
        let factory = SecretFactoryImpl::new();
        let result = factory
            .load_from_env("RIGORIX_TEST_NONEXISTENT_99999", None)
            .await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_load_from_env_not_found_with_fallback() {
        let factory = SecretFactoryImpl::new();
        let result = factory
            .load_from_env(
                "RIGORIX_TEST_NONEXISTENT_99999",
                Some("fallback-value".to_string()),
            )
            .await;
        assert!(result.is_some());
        assert_eq!(result.unwrap().expose(), "fallback-value");
    }

    #[tokio::test]
    async fn test_create_from_value() {
        let factory = SecretFactoryImpl::new();
        let secret = factory.create_from_value("sk-test-abc123");
        assert_eq!(secret.expose(), "sk-test-abc123");
        assert_eq!(format!("{secret:?}"), "[REDACTED]");
    }
}
