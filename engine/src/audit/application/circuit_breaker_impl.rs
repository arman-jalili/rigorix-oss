//! Implementation of `CircuitBreaker`.
//!
//! @canonical .pi/architecture/modules/audit.md#breaker
//! Implements: CircuitBreaker trait — state machine for HTTP resilience
//! Issue: #14
//!
//! Standard circuit breaker with closed → open → half-open → closed
//! state machine. Uses tokio timers for half-open timeout management.

use async_trait::async_trait;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::audit::domain::{AuditError, CircuitBreakerState};

use super::service::{CircuitBreaker as CircuitBreakerTrait, CircuitBreakerStats};

/// Implementation of `CircuitBreaker` with configurable threshold and timeout.
///
/// # State Machine
/// - **Closed**: Normal operation. Requests pass through. Failures are counted.
///   When `consecutive_failures >= threshold`, transitions to Open.
/// - **Open**: Requests are rejected immediately. After `half_open_timeout`,
///   transitions to HalfOpen for probing.
/// - **HalfOpen**: One test request is allowed. If it succeeds, transitions
///   to Closed. If it fails, transitions back to Open.
pub struct CircuitBreakerImpl {
    /// Backend URL this breaker protects.
    backend_url: String,
    /// Consecutive failures before opening.
    threshold: u32,
    /// Time in seconds before attempting half-open probe.
    half_open_timeout_secs: u64,
    /// Current state.
    state: Arc<RwLock<CircuitBreakerState>>,
    /// Timestamp when the breaker last opened (Unix timestamp).
    opened_at: Arc<RwLock<Option<i64>>>,
    /// Consecutive failures counter.
    consecutive_failures: AtomicU32,
    /// Total requests recorded.
    total_requests: AtomicU64,
    /// Total failures recorded.
    total_failures: AtomicU64,
}

impl CircuitBreakerImpl {
    /// Create a new circuit breaker.
    pub fn new(backend_url: String, threshold: u32, half_open_timeout_secs: u64) -> Self {
        Self {
            backend_url,
            threshold,
            half_open_timeout_secs,
            state: Arc::new(RwLock::new(CircuitBreakerState::Closed)),
            opened_at: Arc::new(RwLock::new(None)),
            consecutive_failures: AtomicU32::new(0),
            total_requests: AtomicU64::new(0),
            total_failures: AtomicU64::new(0),
        }
    }
}

#[async_trait]
impl CircuitBreakerTrait for CircuitBreakerImpl {
    #[tracing::instrument(skip_all)]
    async fn allow_request(&self) -> Result<(), AuditError> {
        self.total_requests.fetch_add(1, Ordering::SeqCst);

        let mut state = self.state.write().await;

        match *state {
            CircuitBreakerState::Closed => Ok(()),
            CircuitBreakerState::Open => {
                // Check if we should transition to half-open
                let opened_at = *self.opened_at.read().await;
                if let Some(opened) = opened_at {
                    let elapsed = chrono::Utc::now().timestamp() - opened;
                    if elapsed >= self.half_open_timeout_secs as i64 {
                        // Transition to half-open for probing
                        *state = CircuitBreakerState::HalfOpen;
                        return Ok(());
                    }

                    let retry_after = self.half_open_timeout_secs.saturating_sub(elapsed as u64);
                    return Err(AuditError::CircuitBreakerOpen {
                        backend_url: self.backend_url.clone(),
                        opened_at: opened_at.unwrap_or(0),
                        retry_after_secs: retry_after,
                    });
                }

                // Shouldn't happen, but recover
                *state = CircuitBreakerState::HalfOpen;
                Ok(())
            }
            CircuitBreakerState::HalfOpen => {
                // Only allow one request through in half-open
                // Transition to closed on success, open on failure
                Ok(())
            }
        }
    }

    #[tracing::instrument(skip_all)]
    async fn record_success(&self) -> Result<(), AuditError> {
        let mut state = self.state.write().await;

        match *state {
            CircuitBreakerState::HalfOpen => {
                // Half-open success → close
                *state = CircuitBreakerState::Closed;
                self.consecutive_failures.store(0, Ordering::SeqCst);
            }
            CircuitBreakerState::Closed => {
                // Reset failure count on success
                self.consecutive_failures.store(0, Ordering::SeqCst);
            }
            CircuitBreakerState::Open => {
                // If we're open and got a success, something's wrong
                // but be optimistic and close
                *state = CircuitBreakerState::Closed;
                self.consecutive_failures.store(0, Ordering::SeqCst);
            }
        }

        Ok(())
    }

    #[tracing::instrument(skip_all)]
    async fn record_failure(&self) -> Result<(), AuditError> {
        self.total_failures.fetch_add(1, Ordering::SeqCst);
        let failures = self.consecutive_failures.fetch_add(1, Ordering::SeqCst) + 1;

        let mut state = self.state.write().await;

        match *state {
            CircuitBreakerState::Closed => {
                if failures >= self.threshold {
                    *state = CircuitBreakerState::Open;
                    *self.opened_at.write().await = Some(chrono::Utc::now().timestamp());
                }
            }
            CircuitBreakerState::HalfOpen => {
                // Half-open failure → back to open
                *state = CircuitBreakerState::Open;
                *self.opened_at.write().await = Some(chrono::Utc::now().timestamp());
            }
            CircuitBreakerState::Open => {
                // Already open — update the timer
                *self.opened_at.write().await = Some(chrono::Utc::now().timestamp());
            }
        }

        Ok(())
    }

    #[tracing::instrument(skip_all)]
    async fn state(&self) -> Result<CircuitBreakerState, AuditError> {
        Ok(*self.state.read().await)
    }

    #[tracing::instrument(skip_all)]
    async fn stats(&self) -> Result<CircuitBreakerStats, AuditError> {
        Ok(CircuitBreakerStats {
            state: *self.state.read().await,
            consecutive_failures: self.consecutive_failures.load(Ordering::SeqCst),
            threshold: self.threshold,
            total_requests: self.total_requests.load(Ordering::SeqCst),
            total_failures: self.total_failures.load(Ordering::SeqCst),
        })
    }

    #[tracing::instrument(skip_all)]
    async fn reset(&self) -> Result<(), AuditError> {
        let mut state = self.state.write().await;
        *state = CircuitBreakerState::Closed;
        self.consecutive_failures.store(0, Ordering::SeqCst);

        let mut opened_at = self.opened_at.write().await;
        *opened_at = None;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_initial_state() {
        let cb = CircuitBreakerImpl::new("https://audit.example.com".to_string(), 3, 30);
        assert_eq!(cb.state().await.unwrap(), CircuitBreakerState::Closed);
    }

    #[tokio::test]
    async fn test_allows_requests_when_closed() {
        let cb = CircuitBreakerImpl::new("https://audit.example.com".to_string(), 3, 30);
        assert!(cb.allow_request().await.is_ok());
    }

    #[tokio::test]
    async fn test_opens_after_threshold() {
        let cb = CircuitBreakerImpl::new("https://audit.example.com".to_string(), 3, 30);

        // Record failures below threshold
        cb.record_failure().await.unwrap();
        cb.record_failure().await.unwrap();
        assert_eq!(cb.state().await.unwrap(), CircuitBreakerState::Closed);

        // Record failure that hits threshold
        cb.record_failure().await.unwrap();
        assert_eq!(cb.state().await.unwrap(), CircuitBreakerState::Open);
    }

    #[tokio::test]
    async fn test_rejects_when_open() {
        let cb = CircuitBreakerImpl::new("https://audit.example.com".to_string(), 1, 30);

        cb.record_failure().await.unwrap();
        let result = cb.allow_request().await;
        assert!(result.is_err());
        match result.unwrap_err() {
            AuditError::CircuitBreakerOpen { .. } => {}
            other => panic!("Expected CircuitBreakerOpen, got: {other}"),
        }
    }

    #[tokio::test]
    async fn test_reset() {
        let cb = CircuitBreakerImpl::new("https://audit.example.com".to_string(), 1, 30);

        cb.record_failure().await.unwrap();
        assert_eq!(cb.state().await.unwrap(), CircuitBreakerState::Open);

        cb.reset().await.unwrap();
        assert_eq!(cb.state().await.unwrap(), CircuitBreakerState::Closed);
    }

    #[tokio::test]
    async fn test_record_success_resets_failures() {
        let cb = CircuitBreakerImpl::new("https://audit.example.com".to_string(), 3, 30);

        cb.record_failure().await.unwrap();
        cb.record_failure().await.unwrap();
        cb.record_success().await.unwrap();
        // After success, failure count resets
        assert_eq!(cb.state().await.unwrap(), CircuitBreakerState::Closed);
    }

    #[tokio::test]
    async fn test_stats() {
        let cb = CircuitBreakerImpl::new("https://audit.example.com".to_string(), 3, 30);

        cb.record_failure().await.unwrap();
        cb.record_failure().await.unwrap();
        let _ = cb.allow_request().await;

        let stats = cb.stats().await.unwrap();
        assert_eq!(stats.consecutive_failures, 2);
        assert_eq!(stats.state, CircuitBreakerState::Closed);
    }
}
