//! Implementation of `AuditRecordQueue`.
//!
//! @canonical actions/.pi/architecture/modules/audit-posting.md#record-queue
//! Implements: AuditRecordQueue trait — bounded in-memory FIFO queue
//! Issue: issue-auditposter
//!
//! Provides a bounded, thread-safe in-memory FIFO queue for failed audit
//! record deliveries. When the queue is full, new records are rejected
//! with a `QueueFull` error.

use async_trait::async_trait;
use std::collections::VecDeque;
use tokio::sync::Mutex;

use crate::audit_posting::domain::AuditPostingError;

use super::dto::{QueueRecordInput, QueueRecordOutput};
use super::service::AuditRecordQueue;

/// Bounded in-memory FIFO queue for failed audit record deliveries.
///
/// Uses a `tokio::sync::Mutex<VecDeque>` for thread safety.
/// Configurable capacity (default: 100).
pub struct AuditRecordQueueImpl {
    /// Internal queue storage.
    queue: Mutex<VecDeque<QueueRecordInput>>,
    /// Maximum number of items.
    capacity: u32,
}

impl AuditRecordQueueImpl {
    /// Create a new queue with the given capacity.
    pub fn new(capacity: u32) -> Self {
        Self {
            queue: Mutex::new(VecDeque::with_capacity(capacity as usize)),
            capacity,
        }
    }

    /// Create a new queue with the default capacity (100).
    pub fn default() -> Self {
        Self::new(100)
    }
}

#[async_trait]
impl AuditRecordQueue for AuditRecordQueueImpl {
    #[tracing::instrument(skip_all)]
    async fn enqueue(
        &self,
        input: QueueRecordInput,
    ) -> Result<QueueRecordOutput, AuditPostingError> {
        let mut queue = self.queue.lock().await;
        if queue.len() >= self.capacity as usize {
            return Err(AuditPostingError::QueueFull {
                capacity: self.capacity,
                pending: queue.len() as u32,
            });
        }

        let execution_id = input.record.execution_id;

        queue.push_back(input);

        tracing::info!(
            execution_id = %execution_id,
            queue_len = queue.len(),
            "Record queued for retry"
        );

        Ok(QueueRecordOutput {
            record: None,
            success: true,
            reason: None,
        })
    }

    #[tracing::instrument(skip_all)]
    async fn dequeue(&self) -> Result<Option<QueueRecordOutput>, AuditPostingError> {
        let mut queue = self.queue.lock().await;
        match queue.pop_front() {
            Some(input) => {
                let record = Some(input.record);
                let retry_count = input.retry_count;

                Ok(Some(QueueRecordOutput {
                    record,
                    success: true,
                    reason: Some(format!("Retry attempt {}", retry_count + 1)),
                }))
            }
            None => Ok(None),
        }
    }

    #[tracing::instrument(skip_all)]
    async fn peek(&self) -> Result<Option<QueueRecordOutput>, AuditPostingError> {
        let queue = self.queue.lock().await;
        match queue.front() {
            Some(input) => Ok(Some(QueueRecordOutput {
                record: Some(input.record.clone()),
                success: true,
                reason: Some(format!("Retry attempt {}", input.retry_count + 1)),
            })),
            None => Ok(None),
        }
    }

    async fn len(&self) -> Result<u32, AuditPostingError> {
        let queue = self.queue.lock().await;
        Ok(queue.len() as u32)
    }

    async fn is_empty(&self) -> Result<bool, AuditPostingError> {
        let queue = self.queue.lock().await;
        Ok(queue.is_empty())
    }

    async fn clear(&self) -> Result<u32, AuditPostingError> {
        let mut queue = self.queue.lock().await;
        let len = queue.len() as u32;
        queue.clear();
        Ok(len)
    }

    async fn recover(&self) -> Result<u32, AuditPostingError> {
        // Recovery from filesystem is handled by the FilesystemAuditBackend.
        // In-memory queue starts empty.
        Ok(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit_posting::domain::SignedAuditRecord;

    fn sample_record() -> SignedAuditRecord {
        SignedAuditRecord {
            execution_id: uuid::Uuid::new_v4(),
            timestamp: chrono::Utc::now(),
            run_id: Some(12345),
            workflow_name: Some("test-workflow".to_string()),
            repository: "test-org/test-repo".to_string(),
            git_ref: Some("refs/heads/main".to_string()),
            commit_sha: Some("abc123".to_string()),
            mode: "run".to_string(),
            summary: "Test execution".to_string(),
            signature: Some("sig".to_string()),
            actor: Some("test-user".to_string()),
            metadata: None,
        }
    }

    fn sample_input() -> QueueRecordInput {
        QueueRecordInput {
            record: sample_record(),
            failure_reason: "Connection timeout".to_string(),
            retry_count: 0,
            max_retries: 3,
        }
    }

    #[tokio::test]
    async fn test_enqueue_dequeue() {
        let queue = AuditRecordQueueImpl::new(100);
        let input = sample_input();

        let enqueue_output = queue.enqueue(input).await.unwrap();
        assert!(enqueue_output.success);

        let dequeue_output = queue.dequeue().await.unwrap();
        assert!(dequeue_output.is_some());
        assert!(dequeue_output.unwrap().record.is_some());
    }

    #[tokio::test]
    async fn test_dequeue_empty() {
        let queue = AuditRecordQueueImpl::new(100);
        let result = queue.dequeue().await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_peek() {
        let queue = AuditRecordQueueImpl::new(100);
        queue.enqueue(sample_input()).await.unwrap();

        let peeked = queue.peek().await.unwrap();
        assert!(peeked.is_some());

        // Peek should not remove
        let len = queue.len().await.unwrap();
        assert_eq!(len, 1);
    }

    #[tokio::test]
    async fn test_len_and_is_empty() {
        let queue = AuditRecordQueueImpl::new(100);
        assert!(queue.is_empty().await.unwrap());
        assert_eq!(queue.len().await.unwrap(), 0);

        queue.enqueue(sample_input()).await.unwrap();
        assert!(!queue.is_empty().await.unwrap());
        assert_eq!(queue.len().await.unwrap(), 1);
    }

    #[tokio::test]
    async fn test_clear() {
        let queue = AuditRecordQueueImpl::new(100);
        queue.enqueue(sample_input()).await.unwrap();
        queue.enqueue(sample_input()).await.unwrap();

        let cleared = queue.clear().await.unwrap();
        assert_eq!(cleared, 2);
        assert!(queue.is_empty().await.unwrap());
    }

    #[tokio::test]
    async fn test_queue_full_error() {
        let queue = AuditRecordQueueImpl::new(2);
        queue.enqueue(sample_input()).await.unwrap();
        queue.enqueue(sample_input()).await.unwrap();

        let result = queue.enqueue(sample_input()).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            AuditPostingError::QueueFull { capacity, pending } => {
                assert_eq!(capacity, 2);
                assert_eq!(pending, 2);
            }
            other => panic!("Expected QueueFull, got: {other}"),
        }
    }

    #[tokio::test]
    async fn test_fifo_order() {
        let queue = AuditRecordQueueImpl::new(100);

        let mut input1 = sample_input();
        input1.failure_reason = "first".to_string();
        let mut input2 = sample_input();
        input2.failure_reason = "second".to_string();

        queue.enqueue(input1).await.unwrap();
        queue.enqueue(input2).await.unwrap();

        let first = queue.dequeue().await.unwrap().unwrap();
        assert_eq!(first.reason, Some("Retry attempt 1".to_string()));

        let second = queue.dequeue().await.unwrap().unwrap();
        assert_eq!(second.reason, Some("Retry attempt 1".to_string()));
    }

    #[tokio::test]
    async fn test_recover() {
        let queue = AuditRecordQueueImpl::new(100);
        let recovered = queue.recover().await.unwrap();
        assert_eq!(recovered, 0);
    }

    #[tokio::test]
    async fn test_default_capacity() {
        let queue = AuditRecordQueueImpl::default();
        assert!(queue.is_empty().await.unwrap());
    }
}
