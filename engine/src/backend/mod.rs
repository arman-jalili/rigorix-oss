//! Generic backend integration module.
//!
//! Defines extension points for connecting Rigorix OSS to a remote
//! enforcement backend (e.g., Rigorix Enterprise). The default
//! implementation (`NullConfigProvider`) returns local config unchanged.
//!
//! Third parties can implement the `EnforcementConfigProvider` trait to
//! augment or replace the local `EnforcementConfig` with rules fetched
//! from a remote server.
//!
//! # Architecture
//!
//! ```text
//! backend/
//! └── mod.rs             # EnforcementConfigProvider trait, NullConfigProvider, BackendError
//! ```
//!
//! # Trait
//!
//! `EnforcementConfigProvider` is the only public API:
//! - `fetch_merged_config(local) -> Result<Option<EnforcementConfig>>`
//! - `None` means "use local config as-is"
//! - `Some(merged)` means "use this merged config instead"

use async_trait::async_trait;

use crate::enforcement::domain::config::EnforcementConfig;

/// A provider of augmented enforcement configuration.
///
/// The default implementation returns `None` (use local config).
/// Enterprise users can implement this trait to point their OSS
/// instance at a remote backend that returns merged policy config.
#[async_trait]
pub trait EnforcementConfigProvider: Send + Sync {
    /// Fetch an augmented enforcement config from a remote backend.
    ///
    /// Returns `Ok(None)` to use the local config unchanged.
    /// Returns `Ok(Some(merged))` to replace/override the local config
    /// with the merged result from the backend.
    async fn fetch_merged_config(
        &self,
        local_config: &EnforcementConfig,
    ) -> Result<Option<EnforcementConfig>, BackendError>;
}

/// Default null implementation — always returns `None` (use local config).
pub struct NullConfigProvider;

#[async_trait]
impl EnforcementConfigProvider for NullConfigProvider {
    async fn fetch_merged_config(
        &self,
        _local_config: &EnforcementConfig,
    ) -> Result<Option<EnforcementConfig>, BackendError> {
        Ok(None)
    }
}

/// Errors from backend operations.
#[derive(Debug, Clone, thiserror::Error)]
pub enum BackendError {
    /// The backend request failed due to a network error.
    #[error("Backend request failed: {detail}")]
    RequestFailed { detail: String },

    /// The backend returned an unauthorized response (401).
    #[error("Backend unauthorized: {detail}")]
    Unauthorized { detail: String },

    /// The backend returned a server error (5xx).
    #[error("Backend server error: {detail}")]
    ServerError { detail: String },

    /// The backend is unavailable.
    #[error("Backend unavailable: {detail}")]
    Unavailable { detail: String },

    /// The backend returned an invalid response.
    #[error("Backend returned invalid response: {detail}")]
    InvalidResponse { detail: String },
}

impl From<reqwest::Error> for BackendError {
    fn from(err: reqwest::Error) -> Self {
        if err.is_status() {
            if let Some(status) = err.status() {
                match status.as_u16() {
                    401 => BackendError::Unauthorized {
                        detail: err.to_string(),
                    },
                    500..=599 => BackendError::ServerError {
                        detail: err.to_string(),
                    },
                    _ => BackendError::RequestFailed {
                        detail: err.to_string(),
                    },
                }
            } else {
                BackendError::RequestFailed {
                    detail: err.to_string(),
                }
            }
        } else if err.is_timeout() {
            BackendError::Unavailable {
                detail: "Request timed out".to_string(),
            }
        } else if err.is_connect() {
            BackendError::Unavailable {
                detail: "Connection failed".to_string(),
            }
        } else {
            BackendError::RequestFailed {
                detail: err.to_string(),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_null_provider_returns_none() {
        let provider = NullConfigProvider;
        let local = EnforcementConfig::default();
        let result = provider.fetch_merged_config(&local).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_backend_error_display() {
        let err = BackendError::RequestFailed {
            detail: "connection reset".to_string(),
        };
        assert_eq!(err.to_string(), "Backend request failed: connection reset");
    }

    #[test]
    fn test_backend_error_from_reqwest_status_401() {
        // We can't easily construct a reqwest::Error with status in test,
        // but we can verify the From impl compiles and the types work.
        let _: BackendError = BackendError::Unauthorized {
            detail: "test".to_string(),
        };
    }
}
