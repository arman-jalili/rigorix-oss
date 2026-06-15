//! Implementation of `AuditSender`.
//!
//! @canonical .pi/architecture/modules/audit.md#sender
//! Implements: AuditSender trait — HTTP delivery with retry and circuit breaker
//! Issue: #14
//!
//! Delivers audit envelopes to a remote HTTP backend with configurable retry
//! logic, exponential backoff with jitter, and circuit breaker integration
//! for resilience.

use async_trait::async_trait;
use std::sync::Arc;

use crate::audit::domain::AuditError;

use super::dto::{
    DeliverEnvelopeInput, DeliverEnvelopeOutput, SendEnvelopeInput, SendEnvelopeOutput,
};
use super::service::{AuditSender, CircuitBreaker};

/// Implementation of `AuditSender` with HTTP delivery and circuit breaker.
///
/// Uses reqwest for HTTP calls with configurable timeouts and retry logic.
pub struct AuditSenderImpl {
    /// Circuit breaker for backend resilience.
    circuit_breaker: Option<Arc<dyn CircuitBreaker>>,
    /// Default backend URL (can be overridden per-call).
    default_backend_url: Option<String>,
    /// HTTP client with default timeout.
    client: reqwest::Client,
    /// Default request timeout in seconds.
    default_timeout_secs: u64,
}

impl AuditSenderImpl {
    /// Create a new audit sender.
    pub fn new(
        circuit_breaker: Option<Arc<dyn CircuitBreaker>>,
        default_backend_url: Option<String>,
    ) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            circuit_breaker,
            default_backend_url,
            client,
            default_timeout_secs: 30,
        }
    }

    /// Create a new audit sender with a custom timeout.
    pub fn with_timeout(
        circuit_breaker: Option<Arc<dyn CircuitBreaker>>,
        default_backend_url: Option<String>,
        default_timeout_secs: u64,
    ) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(default_timeout_secs))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            circuit_breaker,
            default_backend_url,
            client,
            default_timeout_secs,
        }
    }

    /// Compute the next backoff delay with jitter.
    #[tracing::instrument(skip_all)]
    fn backoff_delay(attempt: u32, base_secs: u64, max_secs: u64) -> tokio::time::Duration {
        let base = base_secs * 2u64.pow(attempt.saturating_sub(1));
        let delay = base.min(max_secs);
        // Add jitter: up to 25% of the delay
        let jitter = rand::random::<f64>() * (delay as f64 * 0.25);
        tokio::time::Duration::from_secs_f64(delay as f64 + jitter)
    }
}

#[async_trait]
impl AuditSender for AuditSenderImpl {
    #[tracing::instrument(skip_all)]
    async fn send(&self, input: SendEnvelopeInput) -> Result<SendEnvelopeOutput, AuditError> {
        let backend_url = input
            .backend_url
            .as_deref()
            .or(self.default_backend_url.as_deref())
            .ok_or(AuditError::NotConfigured {
                missing_field: "backend_url".to_string(),
            })?;

        // Check circuit breaker
        if let Some(ref cb) = self.circuit_breaker {
            cb.allow_request().await?;
        }

        let timeout_secs = input.timeout_secs.unwrap_or(self.default_timeout_secs);
        let start = std::time::Instant::now();

        // Serialize envelope to JSON
        let body = serde_json::to_string(&input.envelope).map_err(|e| {
            AuditError::SerializationFailed {
                detail: e.to_string(),
            }
        })?;

        // Send HTTP POST
        let response = self
            .client
            .post(backend_url)
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
                    // Success — record in circuit breaker
                    if let Some(ref cb) = self.circuit_breaker {
                        cb.record_success().await.unwrap_or_default();
                    }
                    Ok(SendEnvelopeOutput {
                        success: true,
                        http_status: Some(status),
                        duration_ms,
                        backend_url: backend_url.to_string(),
                    })
                } else {
                    // Non-2xx — record failure
                    if let Some(ref cb) = self.circuit_breaker {
                        cb.record_failure().await.unwrap_or_default();
                    }
                    Err(AuditError::SendFailed {
                        detail: format!("HTTP {}", status),
                        attempt: 1,
                        max_retries: 0,
                        http_status: Some(status),
                    })
                }
            }
            Err(e) => {
                // Network error — record failure
                if let Some(ref cb) = self.circuit_breaker {
                    cb.record_failure().await.unwrap_or_default();
                }
                Err(AuditError::SendFailed {
                    detail: e.to_string(),
                    attempt: 1,
                    max_retries: 0,
                    http_status: None,
                })
            }
        }
    }

    async fn deliver_with_retry(
        &self,
        input: DeliverEnvelopeInput,
    ) -> Result<DeliverEnvelopeOutput, AuditError> {
        let mut last_error: Option<String> = None;
        let mut last_status: Option<u16> = None;
        let total_start = std::time::Instant::now();

        for attempt in 1..=input.max_retries {
            let send_input = SendEnvelopeInput {
                envelope: input.envelope.clone(),
                backend_url: None,
                timeout_secs: Some(30),
            };

            match self.send(send_input).await {
                Ok(output) => {
                    return Ok(DeliverEnvelopeOutput {
                        success: true,
                        attempts: attempt,
                        total_duration_ms: total_start.elapsed().as_millis() as u64,
                        last_http_status: output.http_status,
                        last_error: None,
                    });
                }
                Err(err) => {
                    last_error = Some(err.to_string());
                    if let AuditError::SendFailed { http_status, .. } = &err {
                        last_status = *http_status;
                    }

                    // Don't retry if circuit breaker is open
                    if matches!(err, AuditError::CircuitBreakerOpen { .. }) {
                        return Ok(DeliverEnvelopeOutput {
                            success: false,
                            attempts: attempt,
                            total_duration_ms: total_start.elapsed().as_millis() as u64,
                            last_http_status: last_status,
                            last_error: last_error.clone(),
                        });
                    }

                    // Wait for backoff before next retry
                    if attempt < input.max_retries {
                        let delay = Self::backoff_delay(
                            attempt,
                            input.backoff_base_secs,
                            input.backoff_max_secs,
                        );
                        tokio::time::sleep(delay).await;
                    }
                }
            }
        }

        Ok(DeliverEnvelopeOutput {
            success: false,
            attempts: input.max_retries,
            total_duration_ms: total_start.elapsed().as_millis() as u64,
            last_http_status: last_status,
            last_error,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit::application::circuit_breaker_impl::CircuitBreakerImpl;
    use crate::audit::domain::{EventStatus, ExecutionEventRef};

    #[tracing::instrument(skip_all)]
    fn sample_envelope() -> crate::audit::domain::AuditEnvelope {
        crate::audit::domain::AuditEnvelope {
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

    #[tokio::test]
    async fn test_send_no_backend_configured() {
        let sender = AuditSenderImpl::new(None, None);
        let input = SendEnvelopeInput {
            envelope: sample_envelope(),
            backend_url: None,
            timeout_secs: None,
        };
        let result = sender.send(input).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            AuditError::NotConfigured { missing_field } => {
                assert_eq!(missing_field, "backend_url");
            }
            other => panic!("Expected NotConfigured, got: {other}"),
        }
    }

    #[tokio::test]
    async fn test_get_backend_url_from_default() {
        let sender = AuditSenderImpl::new(None, Some("https://audit.example.com".to_string()));
        let input = SendEnvelopeInput {
            envelope: sample_envelope(),
            backend_url: None,
            timeout_secs: Some(5),
        };
        // This will fail with connection error, not NotConfigured
        let result = sender.send(input).await;
        assert!(result.is_err());
        // Should NOT be NotConfigured — we provided a default
        assert!(!matches!(
            result.unwrap_err(),
            AuditError::NotConfigured { .. }
        ));
    }

    #[tokio::test]
    async fn test_circuit_breaker_rejects_when_open() {
        let cb = Arc::new(CircuitBreakerImpl::new(
            "https://audit.example.com".to_string(),
            1,
            30,
        ));
        // Open the breaker
        cb.record_failure().await.unwrap();

        let sender = AuditSenderImpl::new(Some(cb), Some("https://audit.example.com".to_string()));
        let input = SendEnvelopeInput {
            envelope: sample_envelope(),
            backend_url: None,
            timeout_secs: Some(5),
        };
        let result = sender.send(input).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            AuditError::CircuitBreakerOpen { .. } => {}
            other => panic!("Expected CircuitBreakerOpen, got: {other}"),
        }
    }

    #[tokio::test]
    async fn test_backoff_delay_increasing() {
        let d1 = AuditSenderImpl::backoff_delay(1, 1, 60);
        let d2 = AuditSenderImpl::backoff_delay(2, 1, 60);
        let d3 = AuditSenderImpl::backoff_delay(3, 1, 60);

        // Each subsequent attempt should have a longer base delay
        assert!(d1 < d2, "d1={d1:?} should be less than d2={d2:?}");
        assert!(d2 < d3, "d2={d2:?} should be less than d3={d3:?}");
    }

    #[tokio::test]
    async fn test_backoff_delay_capped() {
        let d1 = AuditSenderImpl::backoff_delay(10, 1, 5);
        let d2 = AuditSenderImpl::backoff_delay(20, 1, 5);

        // Both should be capped at max_secs (5) plus jitter
        assert!(d1.as_secs_f64() <= 6.25); // 5 + 25%
        assert!(d2.as_secs_f64() <= 6.25);
    }
}
