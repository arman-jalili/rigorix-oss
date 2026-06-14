//! Implementation of `AuditService`.
//!
//! @canonical .pi/architecture/modules/audit.md
//! Implements: AuditService trait — orchestrates envelope build and delivery
//! Issue: #14
//!
//! Orchestrates the full audit workflow: building envelopes from execution
//! events, delivering them via `AuditSender`, and managing failed deliveries
//! via `AuditQueue`.

use async_trait::async_trait;
use std::sync::Arc;

use crate::audit::domain::AuditError;

use super::audit_queue_impl::AuditQueueImpl;
use super::audit_sender_impl::AuditSenderImpl;
use super::dto::{BuildEnvelopeInput, BuildEnvelopeOutput, DeliverEnvelopeInput, EnqueueInput};
use super::envelope_factory_impl::AuditEnvelopeFactoryImpl;
use super::factory::AuditEnvelopeFactory;
use super::service::RetryPendingOutput;
use super::service::{AuditQueue, AuditSender, AuditService, AuditStatusOutput};

/// Implementation of `AuditService`.
///
/// Coordinates envelope building, sending, and failed delivery retries.
pub struct AuditServiceImpl {
    /// Factory for building envelopes.
    envelope_factory: Box<dyn AuditEnvelopeFactory>,
    /// Sender for delivering envelopes.
    sender: Arc<dyn AuditSender>,
    /// Queue for failed deliveries.
    queue: Box<dyn AuditQueue>,
    /// Whether audit is enabled.
    enabled: bool,
    /// Default retry configuration.
    max_retries: u32,
    backoff_base_secs: u64,
    backoff_max_secs: u64,
}

impl AuditServiceImpl {
    /// Create a new audit service.
    pub fn new(
        envelope_factory: Box<dyn AuditEnvelopeFactory>,
        sender: Arc<dyn AuditSender>,
        queue: Box<dyn AuditQueue>,
        enabled: bool,
    ) -> Self {
        Self {
            envelope_factory,
            sender,
            queue,
            enabled,
            max_retries: 3,
            backoff_base_secs: 1,
            backoff_max_secs: 60,
        }
    }

    /// Create a new audit service with custom retry config.
    pub fn with_retry_config(
        envelope_factory: Box<dyn AuditEnvelopeFactory>,
        sender: Arc<dyn AuditSender>,
        queue: Box<dyn AuditQueue>,
        enabled: bool,
        max_retries: u32,
        backoff_base_secs: u64,
        backoff_max_secs: u64,
    ) -> Self {
        Self {
            envelope_factory,
            sender,
            queue,
            enabled,
            max_retries,
            backoff_base_secs,
            backoff_max_secs,
        }
    }

    /// Create a default audit service with in-memory queue and no sender.
    ///
    /// Useful for testing.
    pub fn default_test() -> Self {
        Self::new(
            Box::new(AuditEnvelopeFactoryImpl::default()),
            Arc::new(AuditSenderImpl::new(None, None)),
            Box::new(AuditQueueImpl::default()),
            true,
        )
    }
}

#[async_trait]
impl AuditService for AuditServiceImpl {
    async fn build_and_send(
        &self,
        input: BuildEnvelopeInput,
    ) -> Result<BuildEnvelopeOutput, AuditError> {
        if !self.enabled {
            return Ok(BuildEnvelopeOutput {
                envelope: self.envelope_factory.build_envelope(input).await?,
                signed: false,
                event_count: 0,
            });
        }

        // Build the envelope
        let envelope = self.envelope_factory.build_envelope(input).await?;
        let event_count = envelope.events.len();
        let signed = envelope.signature.is_some();

        // Try to send
        let deliver_input = DeliverEnvelopeInput {
            envelope: envelope.clone(),
            max_retries: self.max_retries,
            backoff_base_secs: self.backoff_base_secs,
            backoff_max_secs: self.backoff_max_secs,
        };

        let delivery = self.sender.deliver_with_retry(deliver_input).await;

        match delivery {
            Ok(output) if output.success => {
                // Delivered successfully
            }
            Ok(output) => {
                // Failed after retries — enqueue for later retry
                if let Some(error) = output.last_error {
                    let _ = self
                        .queue
                        .enqueue(EnqueueInput {
                            envelope: envelope.clone(),
                            failure_reason: error,
                            retry_count: output.attempts,
                            max_retries: self.max_retries,
                        })
                        .await;
                }
            }
            Err(_) => {
                // Unexpected error from sender — enqueue
                let _ = self
                    .queue
                    .enqueue(EnqueueInput {
                        envelope: envelope.clone(),
                        failure_reason: "send_error".to_string(),
                        retry_count: 0,
                        max_retries: self.max_retries,
                    })
                    .await;
            }
        }

        Ok(BuildEnvelopeOutput {
            envelope,
            signed,
            event_count,
        })
    }

    async fn retry_pending(&self) -> Result<RetryPendingOutput, AuditError> {
        let mut delivered = 0u32;
        let mut still_pending = 0u32;
        let mut dropped = 0u32;

        while let Some(output) = self.queue.dequeue().await? {
            if let Some(envelope) = output.envelope {
                let deliver_input = DeliverEnvelopeInput {
                    envelope,
                    max_retries: self.max_retries,
                    backoff_base_secs: self.backoff_base_secs,
                    backoff_max_secs: self.backoff_max_secs,
                };

                match self.sender.deliver_with_retry(deliver_input).await {
                    Ok(delivery) if delivery.success => {
                        delivered += 1;
                    }
                    Ok(delivery) if delivery.attempts >= self.max_retries => {
                        dropped += 1;
                    }
                    _ => {
                        still_pending += 1;
                    }
                }
            }
        }

        Ok(RetryPendingOutput {
            delivered,
            still_pending,
            dropped,
        })
    }

    async fn status(&self) -> Result<AuditStatusOutput, AuditError> {
        let pending_count = self.queue.len().await?;
        Ok(AuditStatusOutput {
            pending_count,
            circuit_breaker_state: crate::audit::domain::CircuitBreakerState::Closed,
            backend_available: self.enabled,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit::domain::{EventStatus, ExecutionEventRef};

    fn sample_input() -> BuildEnvelopeInput {
        BuildEnvelopeInput {
            execution_id: uuid::Uuid::new_v4(),
            template_id: "test-template".to_string(),
            planning_prompt: "plan the execution".to_string(),
            events: vec![ExecutionEventRef {
                event_type: "task_completed".to_string(),
                summary: "Test task completed".to_string(),
                occurred_at: chrono::Utc::now(),
                correlation_id: None,
                status: EventStatus::Success,
            }],
            metadata: None,
            sign: false,
        }
    }

    #[tokio::test]
    async fn test_build_and_send_disabled() {
        let service = AuditServiceImpl {
            enabled: false,
            ..AuditServiceImpl::default_test()
        };
        // Should succeed even without sender configured since audit is disabled
        let mut input = sample_input();
        input.sign = false;
        let result = service.build_and_send(input).await;
        assert!(result.is_ok());
        let output = result.unwrap();
        assert_eq!(output.event_count, 0); // events counted only when enabled
        assert!(!output.signed);
    }

    #[tokio::test]
    async fn test_build_and_send_enabled_no_backend() {
        let service = AuditServiceImpl::default_test();
        let mut input = sample_input();
        input.sign = false;
        let result = service.build_and_send(input).await;
        // Should still succeed — building works even if send fails
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_status() {
        let service = AuditServiceImpl::default_test();
        let status = service.status().await.unwrap();
        assert_eq!(status.pending_count, 0);
    }
}
