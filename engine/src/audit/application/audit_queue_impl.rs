//! Implementation of `AuditQueue`.
//!
//! @canonical .pi/architecture/modules/audit.md#queue
//! Implements: AuditQueue trait — in-memory bounded queue for failed deliveries
//! Issue: #14
//!
//! Bounded in-memory FIFO queue for managing failed audit envelope deliveries.
//! When the queue is full, new failed deliveries are rejected with `QueueFull`.
//! Thread-safe via `tokio::sync::Mutex`.

use async_trait::async_trait;
use std::collections::VecDeque;
use tokio::sync::Mutex;

use crate::audit::domain::{AuditEnvelope, AuditError};

use super::dto::{EnqueueInput, EnqueueOutput};
use super::service::AuditQueue;

/// In-memory implementation of `AuditQueue` with bounded capacity.
pub struct AuditQueueImpl {
    /// Maximum queue capacity.
    capacity: u32,
    /// Internal FIFO queue (front = oldest, back = newest).
    queue: Mutex<VecDeque<QueuedEnvelope>>,
}

/// A failed delivery envelope waiting for retry.
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct QueuedEnvelope {
    envelope: AuditEnvelope,
    failure_reason: String,
    retry_count: u32,
    max_retries: u32,
}

impl AuditQueueImpl {
    /// Create a new bounded audit queue.
    pub fn new(capacity: u32) -> Self {
        Self {
            capacity,
            queue: Mutex::new(VecDeque::with_capacity(capacity as usize)),
        }
    }
}

impl Default for AuditQueueImpl {
    fn default() -> Self {
        Self::new(100) // Default capacity of 100
    }
}

#[async_trait]
impl AuditQueue for AuditQueueImpl {
    async fn enqueue(&self, input: EnqueueInput) -> Result<EnqueueOutput, AuditError> {
        let mut queue = self.queue.lock().await;

        if queue.len() >= self.capacity as usize {
            return Err(AuditError::QueueFull {
                capacity: self.capacity,
                pending: queue.len() as u32,
            });
        }

        queue.push_back(QueuedEnvelope {
            envelope: input.envelope,
            failure_reason: input.failure_reason,
            retry_count: input.retry_count,
            max_retries: input.max_retries,
        });

        Ok(EnqueueOutput {
            envelope: None,
            success: true,
            reason: None,
        })
    }

    async fn dequeue(&self) -> Result<Option<EnqueueOutput>, AuditError> {
        let mut queue = self.queue.lock().await;

        match queue.pop_front() {
            Some(queued) => Ok(Some(EnqueueOutput {
                envelope: Some(queued.envelope),
                success: true,
                reason: Some(queued.failure_reason),
            })),
            None => Ok(None),
        }
    }

    async fn peek(&self) -> Result<Option<EnqueueOutput>, AuditError> {
        let queue = self.queue.lock().await;

        match queue.front() {
            Some(queued) => Ok(Some(EnqueueOutput {
                envelope: Some(queued.envelope.clone()),
                success: true,
                reason: Some(queued.failure_reason.clone()),
            })),
            None => Ok(None),
        }
    }

    async fn len(&self) -> Result<u32, AuditError> {
        let queue = self.queue.lock().await;
        Ok(queue.len() as u32)
    }

    async fn is_empty(&self) -> Result<bool, AuditError> {
        let queue = self.queue.lock().await;
        Ok(queue.is_empty())
    }

    async fn clear(&self) -> Result<u32, AuditError> {
        let mut queue = self.queue.lock().await;
        let count = queue.len() as u32;
        queue.clear();
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit::domain::{EventStatus, ExecutionEventRef};

    fn sample_envelope() -> AuditEnvelope {
        AuditEnvelope {
            execution_id: uuid::Uuid::new_v4(),
            timestamp: chrono::Utc::now(),
            template_id: "test".to_string(),
            planning_hash: "abc123".to_string(),
            events: vec![ExecutionEventRef {
                event_type: "test".to_string(),
                summary: "test event".to_string(),
                occurred_at: chrono::Utc::now(),
                correlation_id: None,
                status: EventStatus::Success,
            }],
            signature: None,
        }
    }

    fn sample_input() -> EnqueueInput {
        EnqueueInput {
            envelope: sample_envelope(),
            failure_reason: "connection timeout".to_string(),
            retry_count: 0,
            max_retries: 3,
        }
    }

    #[tokio::test]
    async fn test_enqueue_dequeue() {
        let queue = AuditQueueImpl::new(10);
        assert!(queue.is_empty().await.unwrap());

        queue.enqueue(sample_input()).await.unwrap();
        assert_eq!(queue.len().await.unwrap(), 1);

        let output = queue.dequeue().await.unwrap();
        assert!(output.is_some());
        assert!(queue.is_empty().await.unwrap());
    }

    #[tokio::test]
    async fn test_peek_does_not_remove() {
        let queue = AuditQueueImpl::new(10);
        queue.enqueue(sample_input()).await.unwrap();

        let peeked = queue.peek().await.unwrap();
        assert!(peeked.is_some());
        assert_eq!(queue.len().await.unwrap(), 1);
    }

    #[tokio::test]
    async fn test_queue_full() {
        let queue = AuditQueueImpl::new(2);
        queue.enqueue(sample_input()).await.unwrap();
        queue.enqueue(sample_input()).await.unwrap();

        let result = queue.enqueue(sample_input()).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            AuditError::QueueFull { capacity, pending } => {
                assert_eq!(capacity, 2);
                assert_eq!(pending, 2);
            }
            other => panic!("Expected QueueFull, got: {other}"),
        }
    }

    #[tokio::test]
    async fn test_clear() {
        let queue = AuditQueueImpl::new(10);
        queue.enqueue(sample_input()).await.unwrap();
        queue.enqueue(sample_input()).await.unwrap();

        let cleared = queue.clear().await.unwrap();
        assert_eq!(cleared, 2);
        assert!(queue.is_empty().await.unwrap());
    }

    #[tokio::test]
    async fn test_dequeue_empty() {
        let queue = AuditQueueImpl::new(10);
        let output = queue.dequeue().await.unwrap();
        assert!(output.is_none());
    }
}
