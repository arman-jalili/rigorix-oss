//! Implementation of `AuditRecordFactory`.
//!
//! @canonical actions/.pi/architecture/modules/audit-posting.md#record-factory
//! Implements: AuditRecordFactory trait — HMAC-SHA256 signing and record creation
//! Issue: issue-signedauditrecord
//!
//! Builds `SignedAuditRecord` values from execution metadata and handles
//! HMAC-SHA256 signing for integrity verification. The signature is computed
//! over the canonical JSON serialization of all record fields except the
//! `signature` field itself.

use async_trait::async_trait;

use crate::audit_posting::domain::{AuditPostingError, SignedAuditRecord};

use super::dto::{CreateRecordInput, SignRecordInput, SignRecordOutput};
use super::factory::AuditRecordFactory;

use hmac::{Hmac, Mac};
use sha2::Sha256;

/// HMAC-SHA256 type alias.
type HmacSha256 = Hmac<Sha256>;

/// Implementation of `AuditRecordFactory` with HMAC-SHA256 signing.
///
/// Uses the configured HMAC key (hex-encoded) to sign and verify
/// audit records. The key is set at construction time.
pub struct AuditRecordFactoryImpl {
    /// HMAC signing key (raw bytes).
    signing_key: Option<Vec<u8>>,
    /// Key identifier for the signing key.
    key_id: Option<String>,
}

impl AuditRecordFactoryImpl {
    /// Create a new factory with HMAC signing.
    ///
    /// # Arguments
    ///
    /// * `signing_key_hex` - Hex-encoded HMAC-SHA256 signing key (optional).
    ///   When `None`, signing operations will return `KeyNotAvailable`.
    /// * `key_id` - Optional key identifier for the signing key.
    pub fn new(signing_key_hex: Option<&str>, key_id: Option<String>) -> Self {
        let signing_key = signing_key_hex.and_then(|hex| hex::decode(hex).ok());

        Self {
            signing_key,
            key_id,
        }
    }

    /// Serialize the data fields of a record for signing.
    ///
    /// Excludes the `signature` field to compute the signature over
    /// the actual data. Uses a consistent field order via serde.
    fn serialize_for_signing(record: &SignedAuditRecord) -> Result<Vec<u8>, AuditPostingError> {
        // Create a serialization without the signature field
        let data = serde_json::to_value(record).map_err(|e| {
            AuditPostingError::SerializationFailed {
                detail: e.to_string(),
            }
        })?;

        // Remove the signature field for canonical signing
        let mut data_map = match &data {
            serde_json::Value::Object(map) => map.clone(),
            _ => {
                return Err(AuditPostingError::SerializationFailed {
                    detail: "Expected JSON object for record".to_string(),
                });
            }
        };

        data_map.remove("signature");

        serde_json::to_vec(&data_map).map_err(|e| AuditPostingError::SerializationFailed {
            detail: e.to_string(),
        })
    }

    /// Compute HMAC-SHA256 signature for the given data.
    fn compute_signature(&self, data: &[u8]) -> Result<String, AuditPostingError> {
        let key = self.signing_key.as_ref().ok_or(
            AuditPostingError::KeyNotAvailable {
                detail: "No HMAC signing key configured".to_string(),
            },
        )?;

        let mut mac = HmacSha256::new_from_slice(key).map_err(|e| {
            AuditPostingError::SigningFailed {
                detail: format!("Invalid HMAC key length: {e}"),
            }
        })?;

        mac.update(data);
        let result = mac.finalize();
        let code_bytes = result.into_bytes();

        Ok(hex::encode(code_bytes))
    }
}

impl Default for AuditRecordFactoryImpl {
    fn default() -> Self {
        Self {
            signing_key: None,
            key_id: None,
        }
    }
}

#[async_trait]
impl AuditRecordFactory for AuditRecordFactoryImpl {
    #[tracing::instrument(skip_all)]
    async fn create_record(
        &self,
        input: CreateRecordInput,
    ) -> Result<SignedAuditRecord, AuditPostingError> {
        let mut record = SignedAuditRecord {
            execution_id: input.execution_id,
            timestamp: chrono::Utc::now(),
            run_id: input.run_id,
            workflow_name: input.workflow_name,
            repository: input.repository,
            git_ref: input.git_ref,
            commit_sha: input.commit_sha,
            mode: input.mode,
            summary: input.summary,
            signature: None,
            actor: input.actor,
            metadata: input.metadata,
        };

        // Optionally sign the record
        if input.sign {
            let data = Self::serialize_for_signing(&record)?;
            let signature = self.compute_signature(&data)?;
            record.signature = Some(signature);
        }

        Ok(record)
    }

    #[tracing::instrument(skip_all)]
    async fn sign(&self, input: SignRecordInput) -> Result<SignRecordOutput, AuditPostingError> {
        let data = Self::serialize_for_signing(&input.record)?;
        let signature = self.compute_signature(&data)?;

        let mut signed_record = input.record;
        signed_record.signature = Some(signature.clone());

        Ok(SignRecordOutput {
            record: signed_record,
            signature: signature.clone(),
            key_id: self.key_id.clone().unwrap_or_else(|| "default".to_string()),
        })
    }

    #[tracing::instrument(skip_all)]
    async fn verify(
        &self,
        record: &SignedAuditRecord,
    ) -> Result<bool, AuditPostingError> {
        let stored_signature = match &record.signature {
            Some(sig) => sig.clone(),
            None => {
                return Err(AuditPostingError::SignatureMismatch {
                    expected_prefix: "N/A".to_string(),
                    received_prefix: "none".to_string(),
                });
            }
        };

        let data = Self::serialize_for_signing(record)?;
        let computed_signature = self.compute_signature(&data)?;

        // Use constant-time comparison
        let key = self.signing_key.as_ref().ok_or(
            AuditPostingError::KeyNotAvailable {
                detail: "No HMAC signing key configured for verification".to_string(),
            },
        )?;

        let mut mac = HmacSha256::new_from_slice(key).map_err(|e| {
            AuditPostingError::SigningFailed {
                detail: format!("Invalid HMAC key length: {e}"),
            }
        })?;

        mac.update(&data);

        // Verify using constant-time comparison
        let stored_bytes = hex::decode(&stored_signature).map_err(|e| {
            AuditPostingError::SignatureMismatch {
                expected_prefix: stored_signature.chars().take(8).collect(),
                received_prefix: "invalid hex".to_string(),
            }
        })?;

        let result = mac.verify_slice(&stored_bytes).is_ok();

        if !result {
            let expected_prefix = computed_signature.chars().take(8).collect();
            let received_prefix = stored_signature.chars().take(8).collect();
            return Err(AuditPostingError::SignatureMismatch {
                expected_prefix,
                received_prefix,
            });
        }

        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit_posting::application::dto::CreateRecordInput;

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
            sign: false,
            post_immediately: false,
        }
    }

    fn test_key() -> &'static str {
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
    }

    #[tokio::test]
    async fn test_create_record_without_signing() {
        let factory = AuditRecordFactoryImpl::default();
        let record = factory.create_record(sample_input()).await.unwrap();

        assert_eq!(record.repository, "test-org/test-repo");
        assert_eq!(record.mode, "run");
        assert!(record.signature.is_none());
    }

    #[tokio::test]
    async fn test_create_record_with_signing() {
        let factory = AuditRecordFactoryImpl::new(Some(test_key()), Some("test-key".to_string()));
        let mut input = sample_input();
        input.sign = true;

        let record = factory.create_record(input).await.unwrap();
        assert!(record.signature.is_some());
        assert_eq!(record.signature.as_ref().unwrap().len(), 64); // SHA-256 hex = 64 chars
    }

    #[tokio::test]
    async fn test_sign_and_verify() {
        let factory = AuditRecordFactoryImpl::new(Some(test_key()), Some("test-key".to_string()));
        let record = factory.create_record(sample_input()).await.unwrap();

        // Sign the record
        let sign_input = SignRecordInput {
            record,
            key_id: None,
        };
        let sign_output = factory.sign(sign_input).await.unwrap();
        assert_eq!(sign_output.signature.len(), 64);

        // Verify the signed record
        let valid = factory.verify(&sign_output.record).await.unwrap();
        assert!(valid);
    }

    #[tokio::test]
    async fn test_sign_no_key() {
        let factory = AuditRecordFactoryImpl::default();
        let record = factory.create_record(sample_input()).await.unwrap();

        let sign_input = SignRecordInput {
            record,
            key_id: None,
        };
        let result = factory.sign(sign_input).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            AuditPostingError::KeyNotAvailable { .. } => {}
            other => panic!("Expected KeyNotAvailable, got: {other}"),
        }
    }

    #[tokio::test]
    async fn test_verify_tampered_record() {
        let factory = AuditRecordFactoryImpl::new(Some(test_key()), Some("test-key".to_string()));
        let record = factory.create_record(sample_input()).await.unwrap();

        let sign_input = SignRecordInput {
            record,
            key_id: None,
        };
        let mut sign_output = factory.sign(sign_input).await.unwrap();

        // Tamper with the record
        sign_output.record.summary = "Tampered summary".to_string();

        let result = factory.verify(&sign_output.record).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            AuditPostingError::SignatureMismatch { .. } => {}
            other => panic!("Expected SignatureMismatch, got: {other}"),
        }
    }

    #[tokio::test]
    async fn test_verify_no_signature() {
        let factory = AuditRecordFactoryImpl::new(Some(test_key()), Some("test-key".to_string()));
        let record = factory.create_record(sample_input()).await.unwrap();

        let result = factory.verify(&record).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            AuditPostingError::SignatureMismatch { .. } => {}
            other => panic!("Expected SignatureMismatch, got: {other}"),
        }
    }

    #[tokio::test]
    async fn test_verify_no_key() {
        let factory = AuditRecordFactoryImpl::default();
        let record = factory.create_record(sample_input()).await.unwrap();

        let result = factory.verify(&record).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            AuditPostingError::KeyNotAvailable { .. } => {}
            AuditPostingError::SignatureMismatch { .. } => {} // Also acceptable (no sig + no key)
            other => panic!("Expected KeyNotAvailable or SignatureMismatch, got: {other}"),
        }
    }

    #[tokio::test]
    async fn test_wrong_key_verification() {
        let factory = AuditRecordFactoryImpl::new(Some(test_key()), Some("test-key".to_string()));
        let record = factory.create_record(sample_input()).await.unwrap();

        let sign_input = SignRecordInput {
            record,
            key_id: None,
        };
        let sign_output = factory.sign(sign_input).await.unwrap();

        // Create another factory with a different key
        let wrong_key = "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff";
        let wrong_factory = AuditRecordFactoryImpl::new(Some(wrong_key), Some("wrong-key".to_string()));

        let result = wrong_factory.verify(&sign_output.record).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            AuditPostingError::SignatureMismatch { .. } => {}
            other => panic!("Expected SignatureMismatch, got: {other}"),
        }
    }

    #[tokio::test]
    async fn test_serialize_for_signing_excludes_signature() {
        let factory = AuditRecordFactoryImpl::new(Some(test_key()), Some("test-key".to_string()));
        let mut record = factory.create_record(sample_input()).await.unwrap();

        // Manually set a fake signature
        record.signature = Some("fake_signature_value".to_string());

        let data = AuditRecordFactoryImpl::serialize_for_signing(&record).unwrap();
        let json: serde_json::Value = serde_json::from_slice(&data).unwrap();
        let map = json.as_object().unwrap();
        assert!(!map.contains_key("signature"), "signature field should be excluded");
    }

    #[tokio::test]
    async fn test_deterministic_signing() {
        let factory = AuditRecordFactoryImpl::new(Some(test_key()), Some("test-key".to_string()));
        let input = sample_input();

        // Sign the same data twice
        let record1 = factory.create_record(input.clone()).await.unwrap();
        let record2 = factory.create_record(input).await.unwrap();

        let signed1 = factory.sign(SignRecordInput {
            record: record1,
            key_id: None,
        }).await.unwrap();

        let signed2 = factory.sign(SignRecordInput {
            record: record2,
            key_id: None,
        }).await.unwrap();

        // Different execution_ids mean different records, so signatures differ
        // Test: same record should produce same signature
        let record3 = factory.create_record(sample_input()).await.unwrap();
        let signed3 = factory.sign(SignRecordInput {
            record: record3.clone(),
            key_id: None,
        }).await.unwrap();

        let signed3_again = factory.sign(SignRecordInput {
            record: record3,
            key_id: None,
        }).await.unwrap();

        assert_eq!(signed3.signature, signed3_again.signature,
            "Same record signed twice should produce same signature");
    }

    #[tokio::test]
    async fn test_key_id_in_output() {
        let factory = AuditRecordFactoryImpl::new(Some(test_key()), Some("my-key-1".to_string()));
        let record = factory.create_record(sample_input()).await.unwrap();

        let output = factory.sign(SignRecordInput {
            record,
            key_id: None,
        }).await.unwrap();

        assert_eq!(output.key_id, "my-key-1");
    }

    #[tokio::test]
    async fn test_default_key_id() {
        let factory = AuditRecordFactoryImpl::new(Some(test_key()), None);
        let record = factory.create_record(sample_input()).await.unwrap();

        let output = factory.sign(SignRecordInput {
            record,
            key_id: None,
        }).await.unwrap();

        assert_eq!(output.key_id, "default");
    }
}
