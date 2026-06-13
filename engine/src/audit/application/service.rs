//! Service interfaces (use cases) for the Audit bounded context.
//!
//! @canonical .pi/architecture/modules/audit.md
//! Implements: Contract Freeze — AuditService, AuditSender, AuditQueue, CircuitBreaker traits
//! Issue: #13
//!
//! These traits define the application-level operations for audit envelope
//! creation, delivery with retry, queue management, and circuit breaker
//! resilience. All methods are async and return domain error types.
//!
//! # Contract (Frozen)
//! - Every use case has a corresponding trait method
//! - Input/output types are DTOs defined in `dto/`
//! - All methods are async (use `async-trait` for trait object safety)
//! - No implementation — only contract signatures

use async_trait::async_trait;

use crate::audit::domain::AuditError;

use super::dto::{
    BuildEnvelopeInput, BuildEnvelopeOutput, DeliverEnvelopeInput, DeliverEnvelopeOutput,
    EnqueueInput, EnqueueOutput, SendEnvelopeInput, SendEnvelopeOutput,
};

/// Central audit service for building and sending audit envelopes.
///
/// Orchestrates the full audit workflow: building the envelope from
/// execution events, delivering it via `AuditSender`, and managing
/// failed deliveries via `AuditQueue`.
#[async_trait]
pub trait AuditService: Send + Sync {
    /// Build an audit envelope from execution events and send it.
    ///
    /// If the audit backend is not configured, returns a no-op success.
    /// On delivery failure, automatically enqueues for retry.
    /// Returns the delivery outcome.
    async fn build_and_send(
        &self,
        input: BuildEnvelopeInput,
    ) -> Result<BuildEnvelopeOutput, AuditError>;

    /// Retry all pending envelopes in the delivery queue.
    ///
    /// Returns the number of envelopes successfully delivered
    /// and the number still pending after retry.
    async fn retry_pending(&self) -> Result<RetryPendingOutput, AuditError>;

    /// Get the current queue status (pending count, circuit breaker state).
    async fn status(&self) -> Result<AuditStatusOutput, AuditError>;
}

/// HTTP sender for delivering audit envelopes to the remote backend.
///
/// Implements retry logic with exponential backoff and integrates
/// with the `CircuitBreaker` for resilience.
#[async_trait]
pub trait AuditSender: Send + Sync {
    /// Send an audit envelope to the configured backend.
    ///
    /// Returns success only on HTTP 2xx. All other status codes
    /// are returned as `AuditError::SendFailed` for retry handling.
    /// If the circuit breaker is open, returns `CircuitBreakerOpen` immediately.
    async fn send(
        &self,
        input: SendEnvelopeInput,
    ) -> Result<SendEnvelopeOutput, AuditError>;

    /// Deliver an envelope with retry logic.
    ///
    /// Retries according to the configured policy (max retries, backoff).
    /// Exhausts all retries before returning an error.
    async fn deliver_with_retry(
        &self,
        input: DeliverEnvelopeInput,
    ) -> Result<DeliverEnvelopeOutput, AuditError>;
}

/// Queue for managing failed audit deliveries.
///
/// Provides bounded in-memory queueing with capacity limits.
/// When the queue is full, new failed deliveries are dropped.
#[async_trait]
pub trait AuditQueue: Send + Sync {
    /// Enqueue a failed envelope for later retry.
    ///
    /// Returns `QueueFull` error if the queue is at capacity.
    async fn enqueue(&self, input: EnqueueInput) -> Result<EnqueueOutput, AuditError>;

    /// Dequeue the next pending envelope (FIFO order).
    ///
    /// Returns `None` if the queue is empty.
    async fn dequeue(&self) -> Result<Option<EnqueueOutput>, AuditError>;

    /// Peek at the front of the queue without removing.
    async fn peek(&self) -> Result<Option<EnqueueOutput>, AuditError>;

    /// Get the current queue length.
    async fn len(&self) -> Result<u32, AuditError>;

    /// Whether the queue is empty.
    async fn is_empty(&self) -> Result<bool, AuditError>;

    /// Clear all pending items (e.g. on shutdown).
    async fn clear(&self) -> Result<u32, AuditError>;
}

/// Circuit breaker for resilient HTTP delivery to audit backends.
///
/// Implements the standard closed → open → half-open → closed state machine.
/// Prevents cascading failures when the audit backend is unavailable.
#[async_trait]
pub trait CircuitBreaker: Send + Sync {
    /// Check if the circuit breaker allows a request through.
    ///
    /// Returns `Ok(())` if allowed, `Err(CircuitBreakerOpen)` if open.
    async fn allow_request(&self) -> Result<(), AuditError>;

    /// Record a successful request (resets failure count, moves to closed).
    async fn record_success(&self) -> Result<(), AuditError>;

    /// Record a failed request (may trigger open state).
    async fn record_failure(&self) -> Result<(), AuditError>;

    /// Get the current state.
    async fn state(&self) -> Result<crate::audit::domain::CircuitBreakerState, AuditError>;

    /// Get failure statistics.
    async fn stats(&self) -> Result<CircuitBreakerStats, AuditError>;

    /// Reset the circuit breaker to closed state.
    async fn reset(&self) -> Result<(), AuditError>;
}

/// Circuit breaker failure statistics.
#[derive(Debug, Clone)]
pub struct CircuitBreakerStats {
    /// Current state of the breaker.
    pub state: crate::audit::domain::CircuitBreakerState,
    /// Number of consecutive failures.
    pub consecutive_failures: u32,
    /// Failure threshold before opening.
    pub threshold: u32,
    /// Total requests recorded.
    pub total_requests: u64,
    /// Total failures recorded.
    pub total_failures: u64,
}

/// Output for retry_pending operation.
#[derive(Debug, Clone)]
pub struct RetryPendingOutput {
    /// Number of envelopes successfully delivered.
    pub delivered: u32,
    /// Number of envelopes still pending after retry.
    pub still_pending: u32,
    /// Number of envelopes permanantly dropped due to max retries.
    pub dropped: u32,
}

/// Output for audit status query.
#[derive(Debug, Clone)]
pub struct AuditStatusOutput {
    /// Number of envelopes currently in the retry queue.
    pub pending_count: u32,
    /// Current circuit breaker state.
    pub circuit_breaker_state: crate::audit::domain::CircuitBreakerState,
    /// Whether the audit backend is configured and reachable.
    pub backend_available: bool,
}
