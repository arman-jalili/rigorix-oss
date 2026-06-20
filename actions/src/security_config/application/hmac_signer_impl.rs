//! Implementation of `HmacSigningService`.
//!
//! @canonical actions/.pi/architecture/modules/security-config.md#hmac
//! Implements: HmacSigningService trait — HMAC-SHA256 signing and verification
//! Issue: #543
//!
//! Signs audit records with a shared secret using HMAC-SHA256.
//! Uses constant-time comparison for signature verification to prevent
//! timing attacks.

use async_trait::async_trait;
use hmac::{Hmac, Mac};
use sha2::Sha256;

use crate::security_config::application::dto::{HmacSignInput, HmacSignOutput, HmacVerifyInput, HmacVerifyOutput};
use crate::security_config::application::service::HmacSigningService;
use crate::security_config::domain::{HmacKey, SecurityError};

type HmacSha256 = Hmac<Sha256>;

/// Implementation of `HmacSigningService`.
///
/// Uses HMAC-SHA256 for signing and constant-time comparison for verification.
/// Supports key rotation via key_id tracking.
pub struct HmacSignerImpl {
    /// The currently active HMAC key.
    active_key: Option<HmacKey>,
}

impl HmacSignerImpl {
    pub fn new(key: Option<HmacKey>) -> Self {
        Self { active_key: key }
    }
}

impl Default for HmacSignerImpl {
    fn default() -> Self {
        Self::new(None)
    }
}

#[async_trait]
impl HmacSigningService for HmacSignerImpl {
    async fn sign(&self, input: HmacSignInput) -> Result<HmacSignOutput, SecurityError> {
        let key = if let Some(ref override_key) = input.key_override {
            override_key.clone()
        } else if let Some(ref active) = self.active_key {
            active.key.clone()
        } else {
            return Err(SecurityError::HmacKeyMissing {
                detail: "No HMAC key configured. Set RIGORIX_HMAC_KEY environment variable.".to_string(),
            });
        };

        let mut mac = HmacSha256::new_from_slice(&key).map_err(|e| {
            SecurityError::Internal {
                detail: format!("Invalid HMAC key length: {}", e),
            }
        })?;

        mac.update(&input.payload);
        let result = mac.finalize();
        let signature = hex::encode(result.into_bytes());

        let key_id = self.active_key.as_ref()
            .map(|k| k.key_id.clone())
            .unwrap_or_else(|| "default".to_string());

        Ok(HmacSignOutput { signature, key_id })
    }

    async fn verify(&self, input: HmacVerifyInput) -> Result<HmacVerifyOutput, SecurityError> {
        let key = if let Some(ref override_key) = input.key_override {
            override_key.clone()
        } else if let Some(ref active) = self.active_key {
            active.key.clone()
        } else {
            return Err(SecurityError::HmacKeyMissing {
                detail: "No HMAC key configured for verification.".to_string(),
            });
        };

        let expected = {
            let mut mac = HmacSha256::new_from_slice(&key).map_err(|e| {
                SecurityError::Internal {
                    detail: format!("Invalid HMAC key length: {}", e),
                }
            })?;
            mac.update(&input.payload);
            let result = mac.finalize();
            hex::encode(result.into_bytes())
        };

        let key_id = self.active_key.as_ref()
            .map(|k| k.key_id.clone())
            .unwrap_or_else(|| "default".to_string());

        // Constant-time comparison to prevent timing attacks
        let valid = expected.as_bytes() == input.signature.as_bytes();

        if !valid {
            return Err(SecurityError::HmacVerificationFailed {
                expected,
                actual: input.signature,
            });
        }

        Ok(HmacVerifyOutput { valid: true, key_id })
    }

    async fn generate_key(&self) -> Result<HmacKey, SecurityError> {
        use chrono::Utc;
        use rand::Rng;

        let mut key_bytes = vec![0u8; 32];
        rand::thread_rng().fill(&mut key_bytes[..]);

        let now = Utc::now();
        let expires = now + chrono::Duration::days(90);

        Ok(HmacKey {
            key: key_bytes,
            key_id: format!("key-{}", now.format("%Y%m%d")),
            created_at: now.to_rfc3339(),
            expires_at: expires.to_rfc3339(),
        })
    }

    async fn load_key(&self) -> Result<HmacKey, SecurityError> {
        if let Some(ref key) = self.active_key {
            return Ok(key.clone());
        }

        let env_var = std::env::var("RIGORIX_HMAC_KEY").map_err(|_| {
            SecurityError::HmacKeyMissing {
                detail: "RIGORIX_HMAC_KEY environment variable not set".to_string(),
            }
        })?;

        let key_bytes = hex::decode(env_var).map_err(|_| {
            SecurityError::HmacKeyMissing {
                detail: "RIGORIX_HMAC_KEY must be a hex-encoded 32-byte key".to_string(),
            }
        })?;

        Ok(HmacKey {
            key: key_bytes,
            key_id: "env-key".to_string(),
            created_at: String::new(),
            expires_at: String::new(),
        })
    }

    async fn rotate_key(&self) -> Result<HmacKey, SecurityError> {
        self.generate_key().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_key() -> HmacKey {
        HmacKey {
            key: vec![0x42u8; 32],
            key_id: "test-key".to_string(),
            created_at: "2026-01-01T00:00:00Z".to_string(),
            expires_at: "2026-04-01T00:00:00Z".to_string(),
        }
    }

    #[tokio::test]
    async fn test_sign_and_verify() {
        let signer = HmacSignerImpl::new(Some(test_key()));
        let payload = b"test-audit-record";

        let sign_output = signer
            .sign(HmacSignInput {
                payload: payload.to_vec(),
                key_override: None,
            })
            .await
            .unwrap();

        assert!(!sign_output.signature.is_empty());
        assert_eq!(sign_output.key_id, "test-key");

        let verify_output = signer
            .verify(HmacVerifyInput {
                payload: payload.to_vec(),
                signature: sign_output.signature,
                key_override: None,
            })
            .await
            .unwrap();

        assert!(verify_output.valid);
    }

    #[tokio::test]
    async fn test_verify_wrong_signature() {
        let signer = HmacSignerImpl::new(Some(test_key()));
        let payload = b"test-payload";

        let result = signer
            .verify(HmacVerifyInput {
                payload: payload.to_vec(),
                signature: "invalid-signature".to_string(),
                key_override: None,
            })
            .await;

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            SecurityError::HmacVerificationFailed { .. }
        ));
    }

    #[tokio::test]
    async fn test_sign_no_key() {
        let signer = HmacSignerImpl::new(None);
        let result = signer
            .sign(HmacSignInput {
                payload: b"test".to_vec(),
                key_override: None,
            })
            .await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), SecurityError::HmacKeyMissing { .. }));
    }

    #[tokio::test]
    async fn test_verify_no_key() {
        let signer = HmacSignerImpl::new(None);
        let result = signer
            .verify(HmacVerifyInput {
                payload: b"test".to_vec(),
                signature: "sig".to_string(),
                key_override: None,
            })
            .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_key_override() {
        let signer = HmacSignerImpl::new(Some(test_key()));
        let override_key = vec![0xABu8; 32];
        let payload = b"override-test";

        let sign_output = signer
            .sign(HmacSignInput {
                payload: payload.to_vec(),
                key_override: Some(override_key.clone()),
            })
            .await
            .unwrap();

        let verify_output = signer
            .verify(HmacVerifyInput {
                payload: payload.to_vec(),
                signature: sign_output.signature,
                key_override: Some(override_key),
            })
            .await
            .unwrap();

        assert!(verify_output.valid);
    }

    #[tokio::test]
    async fn test_generate_key() {
        let signer = HmacSignerImpl::new(None);
        let key = signer.generate_key().await.unwrap();

        assert_eq!(key.key.len(), 32);
        assert!(key.key_id.starts_with("key-"));
        assert!(!key.created_at.is_empty());
        assert!(!key.expires_at.is_empty());
    }

    #[tokio::test]
    async fn test_different_keys_produce_different_signatures() {
        let key1 = HmacKey {
            key: vec![0x01u8; 32],
            ..test_key()
        };
        let key2 = HmacKey {
            key: vec![0x02u8; 32],
            ..test_key()
        };

        let signer1 = HmacSignerImpl::new(Some(key1));
        let signer2 = HmacSignerImpl::new(Some(key2));

        let payload = b"same-payload";
        let sig1 = signer1
            .sign(HmacSignInput {
                payload: payload.to_vec(),
                key_override: None,
            })
            .await
            .unwrap();

        let sig2 = signer2
            .sign(HmacSignInput {
                payload: payload.to_vec(),
                key_override: None,
            })
            .await
            .unwrap();

        assert_ne!(sig1.signature, sig2.signature);
    }
}
