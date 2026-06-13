//! Factory interfaces for constructing Audit domain objects.
//!
//! @canonical .pi/architecture/modules/audit.md
//! Implements: Contract Freeze — AuditEnvelopeFactory, CircuitBreakerFactory traits
//! Issue: #13
//!
//! Factories encapsulate the construction of complex domain objects,
//! allowing implementations to inject dependencies and apply defaults
//! without exposing construction logic to callers.
//!
//! # Contract (Frozen)
//! - Every factory method returns a configured domain object
//! - Validation is applied during construction
//! - No mutable state in factory implementations

use async_trait::async_trait;

use crate::audit::domain::{AuditEnvelope, AuditError};

use super::dto::BuildEnvelopeInput;

/// Factory for constructing `AuditEnvelope` values.
///
/// Handles building envelopes from execution events, computing
/// the planning hash, and optionally applying HMAC signing.
#[async_trait]
pub trait AuditEnvelopeFactory: Send + Sync {
    /// Build an `AuditEnvelope` from execution event data.
    ///
    /// Computes the planning hash from the provided template and prompt,
    /// timestamps the envelope with the current time, and optionally
    /// applies HMAC signing if a signing key is configured.
    async fn build_envelope(&self, input: BuildEnvelopeInput) -> Result<AuditEnvelope, AuditError>;

    /// Verify an envelope's HMAC signature.
    ///
    /// Returns `SignatureMismatch` if the signature is invalid or missing.
    async fn verify_signature(&self, envelope: &AuditEnvelope) -> Result<(), AuditError>;
}

/// Factory for constructing `CircuitBreaker` instances.
///
/// Applies default thresholds and timeouts when not explicitly provided.
#[async_trait]
pub trait CircuitBreakerFactory: Send + Sync {
    /// Create a `CircuitBreaker` with the given configuration.
    ///
    /// Returns a boxed `CircuitBreaker` trait object.
    async fn create(
        &self,
        backend_url: String,
        threshold: u32,
        half_open_timeout_secs: u64,
    ) -> Result<Box<dyn super::service::CircuitBreaker>, AuditError>;

    /// Create a `CircuitBreaker` with default values.
    async fn create_default(
        &self,
        backend_url: String,
    ) -> Result<Box<dyn super::service::CircuitBreaker>, AuditError>;
}
