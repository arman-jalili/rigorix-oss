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

    /// Compute HMAC-SHA256 signature over the full canonical envelope.
    ///
    /// GAP-A-06: previously only 7 scalar fields were signed — event contents,
    /// `file_paths`, `scoring_results`, `identity` and git metadata were
    /// unsigned, so tampering with any evidence field went undetected.
    fn compute_signature(envelope: &AuditEnvelope, key: &str) -> Result<String, AuditError> {
        use hmac::Mac;
        let mut mac =
            Hmac::<Sha256>::new_from_slice(key.as_bytes()).map_err(|e| AuditError::Internal {
                detail: format!("HMAC key error: {e}"),
            })?;

        // Canonical form: the FULL envelope serialized as sorted-key JSON.
        // serde_json::Map is BTreeMap-backed (no preserve_order feature), so
        // HashMap fields serialize deterministically. The signature field is
        // excluded so signing and verification hash identical bytes.
        let mut canonical_envelope = envelope.clone();
        canonical_envelope.signature = None;
        let canonical =
            serde_json::to_string(&canonical_envelope).map_err(|e| AuditError::Internal {
                detail: format!("Canonical envelope serialization failed: {e}"),
            })?;
        mac.update(canonical.as_bytes());

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
            source: input.source,
            repository: input.repository,
            author: input.author,
            identity: input.identity,
            total_tokens: input.total_tokens,
            duration_ms: input.duration_ms,
            git_commit: input.git_commit,
            git_branch: input.git_branch,
            model_version: input.model_version,
            planning_prompt: input.planning_prompt_content,
            file_paths: input.file_paths,
            events: input.events,
            scoring_results: input.scoring_results,
            approval_events: Vec::new(),
            scope_violations: Vec::new(),
            decision_context_ref: None,
            signature: None,
            // GAP-M-12: an unsigned run is explicitly degraded evidence.
            // Approval-bearing runs must request signing; this marker makes
            // the absence of a signature observable downstream.
            evidence_degraded: !input.sign,
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
                payload: None,
            }],
            source: None,
            total_tokens: 0,
            duration_ms: 0,
            git_commit: None,
            git_branch: None,
            model_version: None,
            planning_prompt_content: None,
            file_paths: vec![],
            metadata: None,
            scoring_results: std::collections::HashMap::new(),
            sign: false,
            repository: None,
            author: None,
            identity: None,
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

    /// GAP-A-06: the HMAC must cover the FULL envelope. Tampering with any
    /// evidence field — event contents, file_paths, scoring_results — must
    /// break verification. (Previously only 7 scalar fields were signed.)
    #[tokio::test]
    async fn test_verify_signature_detects_evidence_tampering() {
        let factory = AuditEnvelopeFactoryImpl::new(Some("test-key-123".to_string()));

        // Tamper with an event's payload content.
        let mut input = sample_input();
        input.sign = true;
        let mut envelope = factory.build_envelope(input).await.unwrap();
        envelope.events[0].summary = "Tampered summary".to_string();
        assert!(
            factory.verify_signature(&envelope).await.is_err(),
            "tampered event content must fail verification"
        );

        // Tamper with file_paths.
        let mut input = sample_input();
        input.sign = true;
        let mut envelope = factory.build_envelope(input).await.unwrap();
        envelope.file_paths.push("src/injected.rs".to_string());
        assert!(
            factory.verify_signature(&envelope).await.is_err(),
            "tampered file_paths must fail verification"
        );

        // Tamper with scoring_results.
        let mut input = sample_input();
        input.sign = true;
        let mut envelope = factory.build_envelope(input).await.unwrap();
        envelope.scoring_results.insert(
            "node-1".to_string(),
            crate::audit::domain::ScoringResultRef {
                passed: true,
                backend: "test".to_string(),
                dimensions: std::collections::HashMap::new(),
                duration_ms: 10,
            },
        );
        assert!(
            factory.verify_signature(&envelope).await.is_err(),
            "tampered scoring_results must fail verification"
        );

        // Untampered envelope still verifies (control).
        let mut input = sample_input();
        input.sign = true;
        let envelope = factory.build_envelope(input).await.unwrap();
        assert!(factory.verify_signature(&envelope).await.is_ok());
    }

    /// GAP-M-12: unsigned envelopes are explicitly marked as degraded
    /// evidence; signed envelopes are not.
    #[tokio::test]
    async fn test_evidence_degraded_marker() {
        let factory = AuditEnvelopeFactoryImpl::new(Some("test-key-123".to_string()));

        let mut unsigned_input = sample_input();
        unsigned_input.sign = false;
        let unsigned = factory.build_envelope(unsigned_input).await.unwrap();
        assert!(
            unsigned.evidence_degraded,
            "unsigned envelope must be marked evidence_degraded"
        );
        assert!(unsigned.signature.is_none());

        let mut signed_input = sample_input();
        signed_input.sign = true;
        let signed = factory.build_envelope(signed_input).await.unwrap();
        assert!(
            !signed.evidence_degraded,
            "signed envelope must not be marked degraded"
        );
        assert!(signed.signature.is_some());
    }

    /// GAP-L-09: git provenance fields must carry from input into the
    /// envelope (the orchestrator populates them from `detect_git_info`).
    #[tokio::test]
    async fn test_git_provenance_fields_carry_through() {
        let factory = AuditEnvelopeFactoryImpl::default();
        let mut input = sample_input();
        input.git_commit = Some("a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2".to_string());
        input.git_branch = Some("main".to_string());

        let envelope = factory.build_envelope(input).await.unwrap();
        assert_eq!(
            envelope.git_commit.as_deref(),
            Some("a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2")
        );
        assert_eq!(envelope.git_branch.as_deref(), Some("main"));
    }
}
