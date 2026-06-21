//! Implementation of `AuditPostingService`.
//!
//! @canonical actions/.pi/architecture/modules/audit-posting.md#posting-service
//! Implements: AuditPostingService trait — orchestrates audit record creation,
//! signing, posting, and retry workflow
//! Issue: issue-auditposter
//!
//! Orchestrates the full audit posting workflow:
//! 1. Create record via `AuditRecordFactory`
//! 2. Sign with HMAC-SHA256
//! 3. Post to backend via `AuditBackend`
//! 4. On failure, enqueue for retry via `AuditRecordQueue`

use async_trait::async_trait;
use std::sync::Arc;

use crate::audit_posting::domain::AuditPostingError;

use super::dto::{
    CreateRecordInput, CreateRecordOutput, PostRecordInput, PostRecordOutput, SignRecordInput,
    SignRecordOutput, VerifyRecordInput, VerifyRecordOutput,
};
use super::factory::AuditRecordFactory;
use super::service::{AuditPostingService, AuditRecordQueue, PostingStatusOutput, RetryPendingOutput};

use crate::audit_posting::infrastructure::repository::AuditBackend;

/// Implementation of `AuditPostingService`.
///
/// Orchestrates the end-to-end audit posting workflow with retry support.
pub struct AuditPostingServiceImpl {
    /// Factory for creating and signing audit records.
    record_factory: Arc<dyn AuditRecordFactory>,
    /// Backend for posting records.
    backend: Arc<dyn AuditBackend>,
    /// Queue for failed deliveries.
    queue: Arc<dyn AuditRecordQueue>,
    /// Whether to automatically sign records.
    auto_sign: bool,
    /// Maximum retry attempts for posting.
    max_retries: u32,
    /// Base delay between retries in seconds.
    retry_delay_secs: u64,
}

impl AuditPostingServiceImpl {
    /// Create a new audit posting service.
    pub fn new(
        record_factory: Arc<dyn AuditRecordFactory>,
        backend: Arc<dyn AuditBackend>,
        queue: Arc<dyn AuditRecordQueue>,
    ) -> Self {
        Self {
            record_factory,
            backend,
            queue,
            auto_sign: true,
            max_retries: 3,
            retry_delay_secs: 1,
        }
    }

    /// Set whether to automatically sign records before posting.
    pub fn with_auto_sign(mut self, auto_sign: bool) -> Self {
        self.auto_sign = auto_sign;
        self
    }

    /// Set the maximum retry count.
    pub fn with_max_retries(mut self, max_retries: u32) -> Self {
        self.max_retries = max_retries;
        self
    }

    /// Set the base retry delay in seconds.
    pub fn with_retry_delay(mut self, delay_secs: u64) -> Self {
        self.retry_delay_secs = delay_secs;
        self
    }

    /// Attempt to post a record with retry logic.
    async fn post_with_retry_inner(
        &self,
        record: &crate::audit_posting::domain::SignedAuditRecord,
        max_retries: u32,
    ) -> Result<(PostRecordOutput, u32), AuditPostingError> {
        let mut attempts = 0u32;
        let mut last_error = None;

        for attempt in 1..=max_retries {
            attempts = attempt;
            let input = PostRecordInput {
                record: record.clone(),
                backend_url: None,
                timeout_secs: Some(30),
            };

            match self.backend.post(input).await {
                Ok(output) => return Ok((output, attempts)),
                Err(e) => {
                    last_error = Some(e);
                    // Wait before next retry (exponential backoff)
                    if attempt < max_retries {
                        let delay = self.retry_delay_secs * 2u64.pow(attempt.saturating_sub(1));
                        tokio::time::sleep(std::time::Duration::from_secs(delay)).await;
                    }
                }
            }
        }

        Err(last_error.unwrap_or(AuditPostingError::Internal {
            detail: "All retry attempts exhausted".to_string(),
        }))
    }
}

#[async_trait]
impl AuditPostingService for AuditPostingServiceImpl {
    #[tracing::instrument(skip_all)]
    async fn create_and_post(
        &self,
        input: CreateRecordInput,
    ) -> Result<CreateRecordOutput, AuditPostingError> {
        let mut record = self.record_factory.create_record(input).await?;

        // Optionally sign
        if self.auto_sign && record.signature.is_none() {
            let sign_output = self
                .record_factory
                .sign(SignRecordInput {
                    record,
                    key_id: None,
                })
                .await?;
            record = sign_output.record;
        }

        // Post to backend
        let post_result = match self.post_with_retry_inner(&record, self.max_retries).await {
            Ok((output, _attempts)) => {
                tracing::info!(
                    execution_id = %record.execution_id,
                    success = output.success,
                    "Record posted to backend"
                );
                Some(super::dto::PostResultDto {
                    success: output.success,
                    http_status: output.http_status,
                    duration_ms: output.duration_ms,
                    error_detail: if output.success { None } else { Some("Unknown error".to_string()) },
                    attempts: 1,
                })
            }
            Err(e) => {
                tracing::warn!(
                    execution_id = %record.execution_id,
                    error = %e,
                    "Failed to post record, queuing for retry"
                );
                // Enqueue for retry
                self.queue
                    .enqueue(super::dto::QueueRecordInput {
                        record: record.clone(),
                        failure_reason: e.to_string(),
                        retry_count: 0,
                        max_retries: self.max_retries,
                    })
                    .await
                    .ok();

                Some(super::dto::PostResultDto {
                    success: false,
                    http_status: None,
                    duration_ms: 0,
                    error_detail: Some(e.to_string()),
                    attempts: self.max_retries,
                })
            }
        };

        let posted = post_result.as_ref().map_or(false, |r| r.success);

        Ok(CreateRecordOutput {
            record,
            signed: self.auto_sign,
            posted,
            post_result,
        })
    }

    #[tracing::instrument(skip_all)]
    async fn post_record(
        &self,
        input: PostRecordInput,
    ) -> Result<PostRecordOutput, AuditPostingError> {
        self.backend.post(input).await
    }

    #[tracing::instrument(skip_all)]
    async fn sign_record(
        &self,
        input: SignRecordInput,
    ) -> Result<SignRecordOutput, AuditPostingError> {
        self.record_factory.sign(input).await
    }

    #[tracing::instrument(skip_all)]
    async fn verify_record(
        &self,
        input: VerifyRecordInput,
    ) -> Result<VerifyRecordOutput, AuditPostingError> {
        match self.record_factory.verify(&input.record).await {
            Ok(valid) => Ok(VerifyRecordOutput {
                valid,
                key_id: input.key_id,
                detail: Some(if valid { "Signature valid".to_string() } else { "Signature invalid".to_string() }),
            }),
            Err(e) => match e {
                AuditPostingError::SignatureMismatch { .. } => Ok(VerifyRecordOutput {
                    valid: false,
                    key_id: input.key_id,
                    detail: Some(e.to_string()),
                }),
                other => Err(other),
            },
        }
    }

    #[tracing::instrument(skip_all)]
    async fn retry_pending(&self) -> Result<RetryPendingOutput, AuditPostingError> {
        let mut delivered = 0u32;
        let mut dropped = 0u32;
        let mut still_pending = 0u32;

        loop {
            match self.queue.dequeue().await? {
                Some(queued) => {
                    let record = match queued.record {
                        Some(r) => r,
                        None => continue,
                    };

                    // Extract retry count from reason string (format: "Retry attempt N")
                    let retry_count = queued.reason
                        .as_ref()
                        .and_then(|r| r.strip_prefix("Retry attempt "))
                        .and_then(|n| n.parse::<u32>().ok())
                        .unwrap_or(1);

                    if retry_count > 3 {
                        dropped += 1;
                        tracing::warn!(
                            execution_id = %record.execution_id,
                            retries = retry_count,
                            "Record dropped after exhausting retries"
                        );
                        continue;
                    }

                    match self.backend.post(PostRecordInput {
                        record: record.clone(),
                        backend_url: None,
                        timeout_secs: Some(30),
                    }).await {
                        Ok(_output) => {
                            delivered += 1;
                        }
                        Err(_) => {
                            // Re-enqueue for another retry
                            self.queue.enqueue(super::dto::QueueRecordInput {
                                record,
                                failure_reason: format!("Retry attempt {} failed", retry_count),
                                retry_count,
                                max_retries: 3,
                            }).await?;
                            still_pending += 1;
                        }
                    }
                }
                None => break,
            }
        }

        Ok(RetryPendingOutput {
            delivered,
            still_pending,
            dropped,
        })
    }

    #[tracing::instrument(skip_all)]
    async fn status(&self) -> Result<PostingStatusOutput, AuditPostingError> {
        let pending_count = self.queue.len().await?;
        let backend_available = self.backend.health_check().await.unwrap_or(false);

        Ok(PostingStatusOutput {
            pending_count,
            backend_available,
            total_posted: 0, // Tracked externally if needed
            total_failed: 0,
            on_disk_count: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit_posting::application::audit_record_factory_impl::AuditRecordFactoryImpl;
    use crate::audit_posting::application::audit_queue_impl::AuditRecordQueueImpl;
    use crate::audit_posting::domain::SignedAuditRecord;
    use crate::audit_posting::infrastructure::FilesystemAuditBackendImpl;
    use tempfile::TempDir;

    fn test_key() -> &'static str {
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
    }

    fn make_backend(dir: &std::path::Path) -> Arc<dyn AuditBackend> {
        Arc::new(FilesystemAuditBackendImpl::new(dir.to_path_buf()))
    }

    fn make_factory() -> Arc<AuditRecordFactoryImpl> {
        Arc::new(AuditRecordFactoryImpl::new(
            Some(test_key()),
            Some("test-key".to_string()),
        ))
    }

    fn make_queue() -> Arc<AuditRecordQueueImpl> {
        Arc::new(AuditRecordQueueImpl::new(100))
    }

    fn make_service(
        factory: Arc<AuditRecordFactoryImpl>,
        backend: Arc<dyn AuditBackend>,
        queue: Arc<AuditRecordQueueImpl>,
    ) -> AuditPostingServiceImpl {
        AuditPostingServiceImpl::new(factory, backend, queue)
            .with_auto_sign(true)
            .with_max_retries(3)
    }

    fn sample_input() -> CreateRecordInput {
        CreateRecordInput {
            execution_id: uuid::Uuid::new_v4(),
            run_id: Some(12345),
            workflow_name: Some("test-workflow".to_string()),
            repository: "test-org/test-repo".to_string(),
            git_ref: Some("refs/heads/main".to_string()),
            commit_sha: Some("abc123".to_string()),
            mode: "run".to_string(),
            summary: "Test execution".to_string(),
            actor: Some("test-user".to_string()),
            metadata: None,
            sign: true,
            post_immediately: true,
        }
    }

    fn sample_record() -> SignedAuditRecord {
        SignedAuditRecord {
            execution_id: uuid::Uuid::new_v4(),
            timestamp: chrono::Utc::now(),
            run_id: None,
            workflow_name: None,
            repository: "test-org/repo".to_string(),
            git_ref: None,
            commit_sha: None,
            mode: "run".to_string(),
            summary: "test".to_string(),
            signature: None,
            actor: None,
            metadata: None,
        }
    }

    #[tokio::test]
    async fn test_create_and_post_success() {
        let dir = TempDir::new().unwrap();
        let service = make_service(make_factory(), make_backend(dir.path()), make_queue());
        let input = sample_input();

        let output = service.create_and_post(input).await.unwrap();
        assert!(output.signed);
        assert!(output.posted);
        assert_eq!(output.record.repository, "test-org/test-repo");
        assert!(output.record.signature.is_some());
    }

    #[tokio::test]
    async fn test_sign_record() {
        let dir = TempDir::new().unwrap();
        let service = make_service(make_factory(), make_backend(dir.path()), make_queue());
        let record = sample_record();

        let output = service
            .sign_record(SignRecordInput {
                record,
                key_id: None,
            })
            .await
            .unwrap();
        assert_eq!(output.signature.len(), 64);
    }

    #[tokio::test]
    async fn test_verify_valid_signature() {
        let dir = TempDir::new().unwrap();
        let service = make_service(make_factory(), make_backend(dir.path()), make_queue());
        let record = sample_record();

        // Sign the record
        let signed = service
            .sign_record(SignRecordInput {
                record,
                key_id: None,
            })
            .await
            .unwrap();

        // Verify
        let verify_output = service
            .verify_record(VerifyRecordInput {
                record: signed.record,
                key_id: None,
            })
            .await
            .unwrap();
        assert!(verify_output.valid);
    }

    #[tokio::test]
    async fn test_verify_invalid_signature() {
        let dir = TempDir::new().unwrap();
        let service = make_service(make_factory(), make_backend(dir.path()), make_queue());

        let record = SignedAuditRecord {
            signature: Some("invalid_signature_here".to_string()),
            ..sample_record()
        };

        let verify_output = service
            .verify_record(VerifyRecordInput {
                record,
                key_id: None,
            })
            .await
            .unwrap();
        assert!(!verify_output.valid);
    }

    #[tokio::test]
    async fn test_status() {
        let dir = TempDir::new().unwrap();
        let service = make_service(make_factory(), make_backend(dir.path()), make_queue());
        let status = service.status().await.unwrap();
        assert_eq!(status.pending_count, 0);
        assert!(status.backend_available);
    }

    #[tokio::test]
    async fn test_retry_pending_empty() {
        let dir = TempDir::new().unwrap();
        let service = make_service(make_factory(), make_backend(dir.path()), make_queue());
        let result = service.retry_pending().await.unwrap();
        assert_eq!(result.delivered, 0);
        assert_eq!(result.still_pending, 0);
        assert_eq!(result.dropped, 0);
    }

    #[tokio::test]
    async fn test_post_record_direct() {
        let dir = TempDir::new().unwrap();
        let service = make_service(make_factory(), make_backend(dir.path()), make_queue());

        let record = SignedAuditRecord {
            signature: Some("sig".to_string()),
            ..sample_record()
        };

        let output = service
            .post_record(PostRecordInput {
                record,
                backend_url: None,
                timeout_secs: None,
            })
            .await
            .unwrap();
        assert!(output.success);
    }
}
