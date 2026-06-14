//! Implementation of `AuditEnvelopeFactory`.
//!
//! @canonical .pi/architecture/modules/audit.md#envelope
//! Implements: AuditEnvelopeFactory trait — builds envelopes from execution events
//! Issue: #14
//!
//! Builds AuditEnvelope values from execution event data, computes the
//! planning hash for replay verification, and optionally applies HMAC
//! signing for envelope integrity.

use async_trait::async_trait;
use hmac::{Hmac, KeyInit};
use sha2::{Digest, Sha256};

use crate::audit::domain::{AuditEnvelope, AuditError};

use super::dto::BuildEnvelopeInput;
use super::factory::AuditEnvelopeFactory;

/// Implementation of `AuditEnvelopeFactory`.
///
/// Uses SHA-256 for the planning hash and HMAC-SHA256 for optional signing.
pub struct AuditEnvelopeFactoryImpl {
    /// Optional HMAC signing key.
    /// If `None`, envelopes are not signed.
    signing_key: Option<String>,
}

impl AuditEnvelopeFactoryImpl {
    /// Create a new factory with optional HMAC signing.
    pub fn new(signing_key: Option<String>) -> Self {
        Self { signing_key }
    }

    /// Compute the SHA-256 hash of the planning prompt.
    fn compute_planning_hash(planning_prompt: &str) -> String {
        use sha2::digest::FixedOutput;
        let mut hasher = Sha256::new();
        hasher.update(planning_prompt.as_bytes());
        let result = hasher.finalize_fixed();
        let hex: String = result.iter().map(|b| format!("{:02x}", b)).collect();
        hex
    }

    /// Compute HMAC-SHA256 signature over the envelope content.
    fn compute_signature(envelope: &AuditEnvelope, key: &str) -> Result<String, AuditError> {
        use hmac::Mac;
        let mut mac =
            Hmac::<Sha256>::new_from_slice(key.as_bytes()).map_err(|e| AuditError::Internal {
                detail: format!("HMAC key error: {e}"),
            })?;

        // Sign the canonical fields
        mac.update(envelope.execution_id.to_string().as_bytes());
        mac.update(envelope.timestamp.to_rfc3339().as_bytes());
        mac.update(envelope.template_id.as_bytes());
        mac.update(envelope.planning_hash.as_bytes());
        mac.update(&(envelope.events.len() as u64).to_le_bytes());

        let result = mac.finalize().into_bytes();
        let hex: String = result.iter().map(|b| format!("{:02x}", b)).collect();
        Ok(hex)
    }

    /// Set a new signing key (for runtime reconfiguration).
    pub fn set_signing_key(&mut self, key: Option<String>) {
        self.signing_key = key;
    }
}

impl Default for AuditEnvelopeFactoryImpl {
    fn default() -> Self {
        Self::new(None)
    }
}

#[async_trait]
impl AuditEnvelopeFactory for AuditEnvelopeFactoryImpl {
    async fn build_envelope(&self, input: BuildEnvelopeInput) -> Result<AuditEnvelope, AuditError> {
        let planning_hash = Self::compute_planning_hash(&input.planning_prompt);

        let mut envelope = AuditEnvelope {
            execution_id: input.execution_id,
            timestamp: chrono::Utc::now(),
            template_id: input.template_id,
            planning_hash,
            events: input.events,
            signature: None,
        };

        // Optionally apply HMAC signing
        if input.sign {
            if let Some(key) = &self.signing_key {
                let signature = Self::compute_signature(&envelope, key)?;
                envelope.signature = Some(signature);
            } else {
                return Err(AuditError::NotConfigured {
                    missing_field: "signing_key".to_string(),
                });
            }
        }

        Ok(envelope)
    }

    async fn verify_signature(&self, envelope: &AuditEnvelope) -> Result<(), AuditError> {
        let key = self.signing_key.as_ref().ok_or(AuditError::NotConfigured {
            missing_field: "signing_key".to_string(),
        })?;

        let expected = Self::compute_signature(envelope, key)?;

        match &envelope.signature {
            Some(actual) if actual == &expected => Ok(()),
            Some(actual) => Err(AuditError::SignatureMismatch {
                expected_prefix: expected.chars().take(8).collect(),
                received_prefix: actual.chars().take(8).collect(),
            }),
            None => Err(AuditError::SignatureMismatch {
                expected_prefix: expected.chars().take(8).collect(),
                received_prefix: "none".to_string(),
            }),
        }
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
    async fn test_build_envelope() {
        let factory = AuditEnvelopeFactoryImpl::default();
        let input = sample_input();
        let envelope = factory.build_envelope(input).await.unwrap();

        assert_eq!(envelope.template_id, "test-template");
        assert!(envelope.signature.is_none());
        assert_eq!(envelope.events.len(), 1);
    }

    #[tokio::test]
    async fn test_build_envelope_with_signing() {
        let factory = AuditEnvelopeFactoryImpl::new(Some("test-key-123".to_string()));
        let mut input = sample_input();
        input.sign = true;
        let envelope = factory.build_envelope(input).await.unwrap();

        assert!(envelope.signature.is_some());
    }

    #[tokio::test]
    async fn test_build_envelope_signing_without_key_fails() {
        let factory = AuditEnvelopeFactoryImpl::default();
        let mut input = sample_input();
        input.sign = true;
        let result = factory.build_envelope(input).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            AuditError::NotConfigured { missing_field } => {
                assert_eq!(missing_field, "signing_key");
            }
            other => panic!("Expected NotConfigured, got: {other}"),
        }
    }

    #[tokio::test]
    async fn test_verify_signature_valid() {
        let factory = AuditEnvelopeFactoryImpl::new(Some("test-key-123".to_string()));
        let mut input = sample_input();
        input.sign = true;
        let envelope = factory.build_envelope(input).await.unwrap();

        let result = factory.verify_signature(&envelope).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_verify_signature_invalid() {
        let factory = AuditEnvelopeFactoryImpl::new(Some("test-key-123".to_string()));
        let mut input = sample_input();
        input.sign = true;
        let mut envelope = factory.build_envelope(input).await.unwrap();

        // Tamper with the signature
        envelope.signature = Some("tampered".to_string());

        let result = factory.verify_signature(&envelope).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            AuditError::SignatureMismatch { .. } => {}
            other => panic!("Expected SignatureMismatch, got: {other}"),
        }
    }

    #[tokio::test]
    async fn test_planning_hash_consistency() {
        let factory = AuditEnvelopeFactoryImpl::default();
        let input1 = sample_input();
        let input2 = sample_input();

        let e1 = factory.build_envelope(input1).await.unwrap();
        let e2 = factory.build_envelope(input2).await.unwrap();

        // Same planning prompt should produce the same hash
        assert_eq!(e1.planning_hash, e2.planning_hash);
    }
}
