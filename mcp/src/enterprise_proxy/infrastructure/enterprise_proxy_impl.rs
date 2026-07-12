//! Concrete implementation of the EnterpriseProxy aggregate root.
//!
//! @canonical .pi/architecture/modules/enterprise-proxy.md#enterpriseproxy
//! Implements: EnterpriseProxy trait — proxies rigorix_enterprise_* tool calls to enterprise API
//!
//! # EnterpriseProxyImpl
//!
//! Wraps a ProxyClient (HTTP) and SchemaCache to handle enterprise tool calls.
//! Initialization fetches schemas from the enterprise API and caches them.
//! Tool calls are forwarded as JSON-RPC requests and responses are returned.
//!
//! # Contract (Frozen)
//!
//! - All public methods delegate to the EnterpriseProxy trait contract
//! - Thread-safe (Send + Sync) via Arc-internal locking
//! - Never panics — all errors go through ProxyError

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use chrono::Utc;

use crate::enterprise_proxy::domain::entity::{EnterpriseProxy, SchemaCache};
use crate::enterprise_proxy::domain::error::ProxyError;
use crate::enterprise_proxy::domain::value::{
    EnterpriseMetadata, HealthStatus, JsonRpcRequest, ProxyConfig, ToolSchema,
};

// ---------------------------------------------------------------------------
// EnterpriseProxyImpl
// ---------------------------------------------------------------------------

/// Concrete implementation of the EnterpriseProxy aggregate root.
///
/// Wraps a SchemaCache for tool schema caching and delegates HTTP
/// communication to a provided callback or default reqwest client.
///
/// # Invariants
///
/// - SchemaCache is protected by Mutex for thread-safe updates
/// - Configuration is immutable after construction
/// - HTTP errors never cause panics — always return ProxyError
pub struct EnterpriseProxyImpl {
    /// Enterprise connection configuration.
    config: ProxyConfig,

    /// Cached tool schemas (thread-safe).
    schema_cache: Arc<Mutex<SchemaCache>>,

    /// Whether the proxy has been initialized.
    initialized: Arc<Mutex<bool>>,

    /// Reusable HTTP client.
    http_client: reqwest::Client,
}

impl EnterpriseProxyImpl {
    /// Create a new EnterpriseProxyImpl with the given configuration.
    ///
    /// The proxy is not initialized until `initialize()` is called.
    /// Tool calls will return `ProxyError::NotEnabled` until initialized.
    pub fn new(config: ProxyConfig) -> Result<Self, ProxyError> {
        let tls = if config.tls_verify() {
            reqwest::Client::builder().danger_accept_invalid_certs(false)
        } else {
            reqwest::Client::builder().danger_accept_invalid_certs(true)
        };

        let http_client = tls
            .timeout(std::time::Duration::from_secs(config.timeout_secs()))
            .build()
            .map_err(|e| ProxyError::Configuration(format!("HTTP client: {}", e)))?;

        Ok(Self {
            config,
            schema_cache: Arc::new(Mutex::new(SchemaCache::new())),
            initialized: Arc::new(Mutex::new(false)),
            http_client,
        })
    }

    /// Build a JSON-RPC URL from the base API URL.
    fn json_rpc_url(&self) -> Result<String, ProxyError> {
        let base = self.config.api_url().trim_end_matches('/');
        Ok(format!("{}/api/json-rpc", base))
    }

    /// Build a metadata URL from the base API URL.
    fn metadata_url(&self) -> Result<String, ProxyError> {
        let base = self.config.api_url().trim_end_matches('/');
        Ok(format!("{}/api/metadata", base))
    }
}

#[async_trait]
impl EnterpriseProxy for EnterpriseProxyImpl {
    fn is_enabled(&self) -> bool {
        self.initialized.lock().map(|g| *g).unwrap_or(false)
    }

    fn available_tools(&self) -> Vec<ToolSchema> {
        self.schema_cache
            .lock()
            .map(|cache| cache.tools().to_vec())
            .unwrap_or_default()
    }

    fn metadata(&self) -> Option<EnterpriseMetadata> {
        self.schema_cache
            .lock()
            .ok()
            .and_then(|cache| cache.metadata().cloned())
    }

    async fn initialize(&self) -> Result<(), ProxyError> {
        let metadata = self.fetch_metadata().await?;

        let mut cache = self
            .schema_cache
            .lock()
            .map_err(|e| ProxyError::Internal(format!("Schema cache lock poisoned: {}", e)))?;
        cache.update(metadata);

        let mut init = self
            .initialized
            .lock()
            .map_err(|e| ProxyError::Internal(format!("Initialized flag lock poisoned: {}", e)))?;
        *init = true;

        Ok(())
    }

    async fn handle(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, ProxyError> {
        if !self.is_enabled() {
            return Err(ProxyError::NotEnabled);
        }

        let url = self.json_rpc_url()?;

        let request = JsonRpcRequest::new(
            method.to_string(),
            params,
            // Use current timestamp ms as request ID
            Utc::now().timestamp_millis() as u64,
        );

        let response = self
            .http_client
            .post(&url)
            .header(
                "Authorization",
                format!("Bearer {}", self.config.api_key().expose()),
            )
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    ProxyError::Timeout {
                        operation: method.to_string(),
                        timeout_secs: self.config.timeout_secs(),
                    }
                } else if e.is_connect() {
                    ProxyError::Transport(format!("Connection failed: {}", e))
                } else {
                    ProxyError::Transport(e.to_string())
                }
            })?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return match status.as_u16() {
                401 | 403 => Err(ProxyError::Authentication(body)),
                _ => Err(ProxyError::ApiError {
                    status: status.as_u16(),
                    message: body,
                }),
            };
        }

        let json_rpc_response: serde_json::Value = response.json().await.map_err(|e| {
            ProxyError::Deserialization(format!("Failed to parse JSON-RPC response: {}", e))
        })?;

        // Extract the result or error from the JSON-RPC response
        if let Some(result) = json_rpc_response.get("result") {
            Ok(result.clone())
        } else if let Some(err) = json_rpc_response.get("error") {
            let message = err
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("Unknown JSON-RPC error");
            Err(ProxyError::ApiError {
                status: 500,
                message: message.to_string(),
            })
        } else {
            Ok(json_rpc_response)
        }
    }

    async fn health_check(&self) -> Result<HealthStatus, ProxyError> {
        let start = std::time::Instant::now();

        let url = self.metadata_url()?;
        let response = self
            .http_client
            .get(&url)
            .header(
                "Authorization",
                format!("Bearer {}", self.config.api_key().expose()),
            )
            .send()
            .await;

        let latency_ms = start.elapsed().as_millis() as u64;

        match response {
            Ok(resp) => {
                if resp.status().is_success() {
                    if let Ok(metadata) = resp.json::<EnterpriseMetadata>().await {
                        Ok(HealthStatus {
                            healthy: true,
                            latency_ms,
                            version: metadata.version,
                            message: "Enterprise API is healthy".into(),
                        })
                    } else {
                        Ok(HealthStatus {
                            healthy: true,
                            latency_ms,
                            version: "unknown".into(),
                            message: "Enterprise API responded but metadata parse failed".into(),
                        })
                    }
                } else {
                    Ok(HealthStatus {
                        healthy: false,
                        latency_ms,
                        version: "unknown".into(),
                        message: format!("Enterprise API returned status {}", resp.status()),
                    })
                }
            }
            Err(e) => Ok(HealthStatus {
                healthy: false,
                latency_ms,
                version: "unknown".into(),
                message: format!("Enterprise API unreachable: {}", e),
            }),
        }
    }
}

// Private implementation helpers
impl EnterpriseProxyImpl {
    /// Fetch enterprise metadata from the API.
    async fn fetch_metadata(&self) -> Result<EnterpriseMetadata, ProxyError> {
        let url = self.metadata_url()?;

        let response = self
            .http_client
            .get(&url)
            .header(
                "Authorization",
                format!("Bearer {}", self.config.api_key().expose()),
            )
            .send()
            .await
            .map_err(|e| ProxyError::Transport(format!("Failed to fetch schemas: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return match status.as_u16() {
                401 | 403 => Err(ProxyError::Authentication(body)),
                _ => Err(ProxyError::ApiError {
                    status: status.as_u16(),
                    message: body,
                }),
            };
        }

        response
            .json::<EnterpriseMetadata>()
            .await
            .map_err(|e| ProxyError::Deserialization(format!("Failed to parse metadata: {}", e)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> ProxyConfig {
        ProxyConfig::new(
            "https://enterprise.example.com".into(),
            "sk-test-key".into(),
            Some(5),     // short timeout for tests
            Some(false), // disable TLS verify for tests
            Some(1),
            Some(3600),
        )
        .expect("valid test config")
    }

    #[test]
    fn test_enterprise_proxy_impl_creation() {
        let config = test_config();
        let proxy = EnterpriseProxyImpl::new(config);
        assert!(proxy.is_ok());
    }

    #[test]
    fn test_enterprise_proxy_not_enabled_by_default() {
        let config = test_config();
        let proxy = EnterpriseProxyImpl::new(config).unwrap();
        assert!(!proxy.is_enabled());
        assert!(proxy.available_tools().is_empty());
        assert!(proxy.metadata().is_none());
    }

    #[test]
    fn test_enterprise_proxy_impl_is_send_sync() {
        let config = test_config();
        let proxy = EnterpriseProxyImpl::new(config).unwrap();
        // Verify Send + Sync via Arc<dyn EnterpriseProxy>
        let _shared: Arc<dyn EnterpriseProxy> = Arc::new(proxy);
    }
}
