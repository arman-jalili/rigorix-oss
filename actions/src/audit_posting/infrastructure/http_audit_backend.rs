//! HTTP implementation of `AuditBackend`.
//!
//! @canonical actions/.pi/architecture/modules/audit-posting.md#http-audit-backend
//! Implements: AuditBackend trait — HTTP delivery for audit records
//! Issue: issue-auditbackend-trait-open-core-boundary-
//!
//! Delivers signed audit records to a remote HTTP backend. Supports
//! configurable timeouts and backend URL overrides per request.

use async_trait::async_trait;

use crate::audit_posting::domain::{AuditPostingError, SignedAuditRecord};

use crate::audit_posting::application::dto::{
    LoadRecordInput, LoadRecordOutput, PostRecordInput, PostRecordOutput,
};

use super::repository::AuditBackend;

/// HTTP implementation of `AuditBackend`.
///
/// Posts signed audit records via HTTP POST to a configurable backend URL.
/// Uses reqwest for HTTP calls with configurable timeouts.
///
/// Note: This is a minimal HTTP sender. The load/list/delete/count/prune
/// operations are not supported over HTTP (the backend is expected to
/// provide its own management interface).
#[derive(Debug)]
pub struct HttpAuditBackend {
    /// Default backend URL (can be overridden per-call).
    default_backend_url: Option<String>,
    /// HTTP client.
    client: reqwest::Client,
    /// Default request timeout in seconds.
    default_timeout_secs: u64,
}

impl HttpAuditBackend {
    /// Create a new HTTP audit backend.
    pub fn new(default_backend_url: Option<String>) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            default_backend_url,
            client,
            default_timeout_secs: 30,
        }
    }

    /// Create a new HTTP audit backend with a custom timeout.
    pub fn with_timeout(default_backend_url: Option<String>, default_timeout_secs: u64) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(default_timeout_secs))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            default_backend_url,
            client,
            default_timeout_secs,
        }
    }

    /// Resolve the target URL for a post request.
    fn resolve_url(&self, input: &PostRecordInput) -> Result<String, AuditPostingError> {
        input
            .backend_url
            .clone()
            .or_else(|| self.default_backend_url.clone())
            .ok_or(AuditPostingError::NotConfigured {
                missing_field: "backend_url".to_string(),
            })
    }
}

#[async_trait]
impl AuditBackend for HttpAuditBackend {
    #[tracing::instrument(skip_all)]
    async fn post(&self, input: PostRecordInput) -> Result<PostRecordOutput, AuditPostingError> {
        let backend_url = self.resolve_url(&input)?;
        let timeout_secs = input.timeout_secs.unwrap_or(self.default_timeout_secs);
        let start = std::time::Instant::now();

        // Serialize record to JSON
        let body = serde_json::to_string(&input.record).map_err(|e| {
            AuditPostingError::SerializationFailed {
                detail: e.to_string(),
            }
        })?;

        // Send HTTP POST
        let response = self
            .client
            .post(&backend_url)
            .header("Content-Type", "application/json")
            .body(body)
            .timeout(std::time::Duration::from_secs(timeout_secs))
            .send()
            .await;

        let duration_ms = start.elapsed().as_millis() as u64;

        match response {
            Ok(resp) => {
                let status = resp.status().as_u16();
                if (200..300).contains(&status) {
                    Ok(PostRecordOutput {
                        success: true,
                        http_status: Some(status),
                        duration_ms,
                        backend_url,
                    })
                } else {
                    Err(AuditPostingError::PostFailed {
                        detail: format!("HTTP {}", status),
                        attempt: 1,
                        max_retries: 0,
                        http_status: Some(status),
                    })
                }
            }
            Err(e) => {
                if e.is_timeout() {
                    Err(AuditPostingError::BackendUnavailable {
                        backend_url,
                        detail: format!("Request timed out after {timeout_secs}s"),
                        is_transient: true,
                    })
                } else if e.is_connect() {
                    Err(AuditPostingError::BackendUnavailable {
                        backend_url,
                        detail: format!("Connection failed: {e}"),
                        is_transient: true,
                    })
                } else {
                    Err(AuditPostingError::PostFailed {
                        detail: e.to_string(),
                        attempt: 1,
                        max_retries: 0,
                        http_status: None,
                    })
                }
            }
        }
    }

    async fn load(&self, _input: LoadRecordInput) -> Result<LoadRecordOutput, AuditPostingError> {
        // HTTP backend does not support loading individual records
        // without a dedicated retrieval endpoint.
        Err(AuditPostingError::NotConfigured {
            missing_field: "load_endpoint".to_string(),
        })
    }

    async fn list(
        &self,
        _since: Option<chrono::DateTime<chrono::Utc>>,
        _until: Option<chrono::DateTime<chrono::Utc>>,
        _limit: Option<u32>,
    ) -> Result<Vec<SignedAuditRecord>, AuditPostingError> {
        Err(AuditPostingError::NotConfigured {
            missing_field: "list_endpoint".to_string(),
        })
    }

    async fn delete(&self, _execution_id: &uuid::Uuid) -> Result<(), AuditPostingError> {
        Err(AuditPostingError::NotConfigured {
            missing_field: "delete_endpoint".to_string(),
        })
    }

    #[tracing::instrument(skip_all)]
    async fn health_check(&self) -> Result<bool, AuditPostingError> {
        match &self.default_backend_url {
            Some(url) => {
                let start = std::time::Instant::now();
                match self
                    .client
                    .head(url)
                    .timeout(std::time::Duration::from_secs(10))
                    .send()
                    .await
                {
                    Ok(resp) => {
                        let healthy = resp.status().is_success();
                        tracing::debug!(
                            backend_url = %url,
                            healthy = healthy,
                            duration_ms = start.elapsed().as_millis() as u64,
                            "Health check completed"
                        );
                        Ok(healthy)
                    }
                    Err(e) => {
                        tracing::warn!(
                            backend_url = %url,
                            error = %e,
                            "Health check failed"
                        );
                        Ok(false)
                    }
                }
            }
            None => Ok(false),
        }
    }

    async fn count(
        &self,
        _since: Option<chrono::DateTime<chrono::Utc>>,
        _until: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Result<u64, AuditPostingError> {
        Err(AuditPostingError::NotConfigured {
            missing_field: "count_endpoint".to_string(),
        })
    }

    async fn prune(
        &self,
        _older_than: chrono::DateTime<chrono::Utc>,
    ) -> Result<u64, AuditPostingError> {
        Err(AuditPostingError::NotConfigured {
            missing_field: "prune_endpoint".to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_record() -> SignedAuditRecord {
        SignedAuditRecord {
            execution_id: uuid::Uuid::new_v4(),
            timestamp: chrono::Utc::now(),
            run_id: Some(12345),
            workflow_name: Some("test-workflow".to_string()),
            repository: "test-org/test-repo".to_string(),
            git_ref: Some("refs/heads/main".to_string()),
            commit_sha: Some("abc123def456".to_string()),
            mode: "run".to_string(),
            summary: "Test execution".to_string(),
            signature: Some("abcd1234signature".to_string()),
            actor: Some("test-user".to_string()),
            metadata: None,
        }
    }

    #[tokio::test]
    async fn test_post_no_backend_configured() {
        let backend = HttpAuditBackend::new(None);
        let input = PostRecordInput {
            record: sample_record(),
            backend_url: None,
            timeout_secs: None,
        };
        let result = backend.post(input).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            AuditPostingError::NotConfigured { missing_field } => {
                assert_eq!(missing_field, "backend_url");
            }
            other => panic!("Expected NotConfigured, got: {other}"),
        }
    }

    #[tokio::test]
    async fn test_load_not_supported() {
        let backend = HttpAuditBackend::new(Some("https://audit.example.com".to_string()));
        let input = LoadRecordInput {
            execution_id: uuid::Uuid::new_v4(),
            verify_signature: false,
        };
        let result = backend.load(input).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            AuditPostingError::NotConfigured { missing_field } => {
                assert_eq!(missing_field, "load_endpoint");
            }
            other => panic!("Expected NotConfigured, got: {other}"),
        }
    }

    #[tokio::test]
    async fn test_health_check_no_url() {
        let backend = HttpAuditBackend::new(None);
        let healthy = backend.health_check().await.unwrap();
        assert!(!healthy);
    }

    #[tokio::test]
    async fn test_health_check_with_url() {
        let backend = HttpAuditBackend::new(Some("https://audit.example.com".to_string()));
        // Will return false since there's no real server, but should not error
        let healthy = backend.health_check().await.unwrap();
        assert!(!healthy);
    }

    #[tokio::test]
    async fn test_list_not_supported() {
        let backend = HttpAuditBackend::new(Some("https://audit.example.com".to_string()));
        let result = backend.list(None, None, None).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_delete_not_supported() {
        let backend = HttpAuditBackend::new(Some("https://audit.example.com".to_string()));
        let result = backend.delete(&uuid::Uuid::new_v4()).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_count_not_supported() {
        let backend = HttpAuditBackend::new(Some("https://audit.example.com".to_string()));
        let result = backend.count(None, None).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_prune_not_supported() {
        let backend = HttpAuditBackend::new(Some("https://audit.example.com".to_string()));
        let result = backend.prune(chrono::Utc::now()).await;
        assert!(result.is_err());
    }
}
