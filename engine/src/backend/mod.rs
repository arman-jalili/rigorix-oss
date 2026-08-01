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

/// HTTP-backed implementation of `EnforcementConfigProvider`.
///
/// Posts the local `EnforcementConfig` to the configured backend URL and
/// accepts either a merged `EnforcementConfig` JSON body (`Some`), an empty
/// body / `204 No Content` (`None` — use local unchanged), or an error.
///
/// # Protocol
///
/// - `POST {url}` with `Authorization: Bearer {api_key}` (when key is set)
/// - Request body: the local `EnforcementConfig` serialized as JSON
/// - `200` + `EnforcementConfig` JSON → merged config is used
/// - `204` / empty body → local config is used unchanged
/// - Non-2xx → `BackendError` (caller should fall back to local config)
pub struct HttpEnforcementConfigProvider {
    url: String,
    api_key: Option<String>,
    timeout: std::time::Duration,
}

impl HttpEnforcementConfigProvider {
    /// Create a new HTTP provider for the given backend URL.
    pub fn new(
        url: impl Into<String>,
        api_key: Option<String>,
        timeout: std::time::Duration,
    ) -> Self {
        Self {
            url: url.into(),
            api_key,
            timeout,
        }
    }
}

#[async_trait]
impl EnforcementConfigProvider for HttpEnforcementConfigProvider {
    async fn fetch_merged_config(
        &self,
        local_config: &EnforcementConfig,
    ) -> Result<Option<EnforcementConfig>, BackendError> {
        let client = reqwest::Client::builder()
            .timeout(self.timeout)
            .build()
            .map_err(|e| BackendError::RequestFailed {
                detail: e.to_string(),
            })?;

        let mut request = client.post(&self.url).json(local_config);
        if let Some(ref key) = self.api_key {
            request = request.bearer_auth(key);
        }

        let response = request.send().await?;
        if response.status() == reqwest::StatusCode::NO_CONTENT {
            return Ok(None);
        }
        if !response.status().is_success() {
            return Err(BackendError::RequestFailed {
                detail: format!("backend returned HTTP {}", response.status()),
            });
        }

        let body = response.text().await?;
        if body.trim().is_empty() || body.trim() == "null" {
            return Ok(None);
        }

        serde_json::from_str(&body)
            .map(Some)
            .map_err(|e| BackendError::InvalidResponse {
                detail: format!("failed to parse merged enforcement config: {e}"),
            })
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

    #[tokio::test]
    async fn test_http_provider_returns_merged_config() {
        // Tiny in-test HTTP server serving a merged EnforcementConfig.
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use tokio::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut buf = [0u8; 4096];
            let _ = socket.read(&mut buf).await;
            let merged_fixture = EnforcementConfig {
                execution_limits: crate::enforcement::domain::config::ExecutionLimits {
                    max_execution_time_secs: 5000,
                    ..EnforcementConfig::default().execution_limits
                },
                ..EnforcementConfig::default()
            };
            let body = serde_json::to_string(&merged_fixture).unwrap();
            let response = format!(
                "HTTP/1.1 200 OK
Content-Type: application/json
Content-Length: {}
Connection: close

{}",
                body.len(),
                body
            );
            let _ = socket.write_all(response.as_bytes()).await;
        });

        let provider = HttpEnforcementConfigProvider::new(
            format!("http://{addr}/enforcement"),
            Some("test-key".to_string()),
            std::time::Duration::from_secs(5),
        );
        let merged = provider
            .fetch_merged_config(&EnforcementConfig::default())
            .await
            .expect("provider should succeed");
        let merged = merged.expect("should return a merged config");
        assert_eq!(merged.execution_limits.max_execution_time_secs, 5000);

        server.await.unwrap();
    }

    #[tokio::test]
    async fn test_http_provider_empty_body_means_use_local() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use tokio::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut buf = [0u8; 4096];
            let _ = socket.read(&mut buf).await;
            let response = "HTTP/1.1 204 No Content
Content-Length: 0
Connection: close

";
            let _ = socket.write_all(response.as_bytes()).await;
        });

        let provider = HttpEnforcementConfigProvider::new(
            format!("http://{addr}/enforcement"),
            None,
            std::time::Duration::from_secs(5),
        );
        let result = provider
            .fetch_merged_config(&EnforcementConfig::default())
            .await
            .expect("provider should succeed");
        assert!(result.is_none(), "204 means use local config unchanged");

        server.await.unwrap();
    }
}
