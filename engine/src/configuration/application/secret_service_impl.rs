//! Implementation of `SecretService`.
//!
//! @canonical .pi/architecture/modules/configuration.md#secret
//! Implements: SecretService trait — load and validate secrets
//! Issue: #4

use async_trait::async_trait;

use crate::configuration::application::dto::{LoadSecretInput, LoadSecretOutput};
use crate::configuration::application::factory::SecretFactory;
use crate::configuration::application::service::SecretService;
use crate::configuration::domain::ConfigurationError;
use crate::configuration::infrastructure::secret_factory_impl::SecretFactoryImpl;

/// Implementation of `SecretService` using `SecretFactory` for loading.
pub struct SecretServiceImpl {
    factory: Box<dyn SecretFactory>,
}

impl SecretServiceImpl {
    pub fn new(factory: Box<dyn SecretFactory>) -> Self {
        Self { factory }
    }
}

impl Default for SecretServiceImpl {
    #[tracing::instrument(skip_all)]
    fn default() -> Self {
        Self::new(Box::new(SecretFactoryImpl::new()))
    }
}

#[async_trait]
impl SecretService for SecretServiceImpl {
    #[tracing::instrument(skip_all)]
    async fn load(&self, input: LoadSecretInput) -> Result<LoadSecretOutput, ConfigurationError> {
        let secret = self
            .factory
            .load_from_env(&input.env_var, input.fallback.clone())
            .await;

        match secret {
            Some(secret) => {
                let source = if std::env::var(&input.env_var).is_ok() {
                    format!("env:{}", input.env_var)
                } else {
                    "fallback".to_string()
                };
                Ok(LoadSecretOutput { secret, source })
            }
            None => {
                if input.required {
                    Err(ConfigurationError::EnvVarError {
                        var: input.env_var.clone(),
                        detail: "Environment variable not set and no fallback provided".to_string(),
                    })
                } else {
                    Ok(LoadSecretOutput {
                        secret: crate::configuration::domain::Secret::new(""),
                        source: "none".to_string(),
                    })
                }
            }
        }
    }

    async fn validate_required(
        &self,
        required_keys: Vec<String>,
    ) -> Result<Vec<String>, ConfigurationError> {
        let mut missing = Vec::new();
        for key in required_keys {
            match std::env::var(&key) {
                Ok(value) => {
                    if value.is_empty() {
                        missing.push(key);
                    }
                }
                Err(_) => {
                    missing.push(key);
                }
            }
        }
        Ok(missing)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::configuration::infrastructure::secret_factory_impl::SecretFactoryImpl;

    #[tracing::instrument(skip_all)]
    fn create_service() -> SecretServiceImpl {
        SecretServiceImpl::new(Box::new(SecretFactoryImpl::new()))
    }

    #[tokio::test]
    async fn test_load_secret_found() {
        let service = create_service();
        let input = LoadSecretInput {
            env_var: "PATH".to_string(),
            fallback: None,
            required: false,
        };
        let output = service.load(input).await.unwrap();
        assert!(!output.secret.is_empty());
        assert!(output.source.starts_with("env:"));
    }

    #[tokio::test]
    async fn test_load_secret_not_found_not_required() {
        let service = create_service();
        let input = LoadSecretInput {
            env_var: "RIGORIX_TEST_NONEXISTENT_99999".to_string(),
            fallback: None,
            required: false,
        };
        let output = service.load(input).await.unwrap();
        assert!(output.secret.is_empty());
        assert_eq!(output.source, "none");
    }

    #[tokio::test]
    async fn test_load_secret_not_found_required() {
        let service = create_service();
        let input = LoadSecretInput {
            env_var: "RIGORIX_TEST_NONEXISTENT_99999".to_string(),
            fallback: None,
            required: true,
        };
        let result = service.load(input).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_load_secret_with_fallback() {
        let service = create_service();
        let input = LoadSecretInput {
            env_var: "RIGORIX_TEST_NONEXISTENT_99999".to_string(),
            fallback: Some("fallback-key".to_string()),
            required: false,
        };
        let output = service.load(input).await.unwrap();
        assert_eq!(output.secret.expose(), "fallback-key");
        assert_eq!(output.source, "fallback");
    }

    #[tokio::test]
    async fn test_validate_required_all_present() {
        let service = create_service();
        let missing = service
            .validate_required(vec!["PATH".to_string()])
            .await
            .unwrap();
        assert!(missing.is_empty());
    }

    #[tokio::test]
    async fn test_validate_required_some_missing() {
        let service = create_service();
        let missing = service
            .validate_required(vec![
                "PATH".to_string(),
                "RIGORIX_TEST_NONEXISTENT_99999".to_string(),
            ])
            .await
            .unwrap();
        assert_eq!(missing.len(), 1);
        assert_eq!(missing[0], "RIGORIX_TEST_NONEXISTENT_99999");
    }
}
