//! Implementation of `SecretMaskingService`.
//!
//! @canonical actions/.pi/architecture/modules/security-config.md#masker
//! Implements: SecretMaskingService trait — masks secrets from workflow logs
//! Issue: #540
//!
//! Uses GitHub Actions `::add-mask::<value>` workflow commands. Once masked,
//! the value is replaced with `***` in all subsequent log output.
//! Must be called BEFORE any logging.

use async_trait::async_trait;

use crate::security_config::application::dto::{MaskSecretsInput, MaskSecretsOutput};
use crate::security_config::application::service::SecretMaskingService;
use crate::security_config::domain::SecurityError;

/// Known secret patterns for log scrubbing detection.
const SECRET_PATTERNS: &[&str] = &[
    "sk-",   // OpenAI API keys
    "ghp_",  // GitHub personal access tokens
    "gho_",  // GitHub OAuth tokens
    "ghu_",  // GitHub user tokens
    "ghs_",  // GitHub app tokens
    "ghr_",  // GitHub refresh tokens
    "xoxb-", // Slack bot tokens
    "xoxp-", // Slack user tokens
    "AKIA",  // AWS access keys
];

/// Implementation of `SecretMaskingService`.
///
/// Masks secrets using GitHub Actions workflow commands.
/// In non-CI environments, masking is simulated via log statements.
pub struct SecretMaskerImpl {
    /// Whether we're running in a GitHub Actions CI environment.
    is_ci: bool,
}

impl SecretMaskerImpl {
    pub fn new() -> Self {
        let is_ci = std::env::var("GITHUB_ACTIONS").is_ok();
        Self { is_ci }
    }

    /// Create a new instance with explicit CI flag (for testing).
    pub fn with_ci(is_ci: bool) -> Self {
        Self { is_ci }
    }
}

impl Default for SecretMaskerImpl {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SecretMaskingService for SecretMaskerImpl {
    async fn mask(&self, secret: &str) -> Result<(), SecurityError> {
        if secret.is_empty() {
            return Ok(());
        }

        if self.is_ci {
            // In CI, use GitHub Actions workflow command
            println!("::add-mask::{}", secret);
        } else {
            // In local dev, log that masking would occur
            let hint = if secret.len() > 8 {
                format!("{}...", &secret[..8])
            } else {
                "****".to_string()
            };
            tracing::info!(secret_hint = %hint, "Masking secret (CI only)");
        }

        Ok(())
    }

    async fn mask_all(&self, input: MaskSecretsInput) -> Result<MaskSecretsOutput, SecurityError> {
        let mut masked_count = 0u32;
        let mut masked_hints = Vec::new();

        for secret in &input.secrets {
            if secret.is_empty() {
                continue;
            }
            self.mask(secret).await?;
            masked_count += 1;
            masked_hints.push(Self::hint(secret));
        }

        Ok(MaskSecretsOutput {
            masked_count,
            masked_hints,
        })
    }

    async fn contains_secret(&self, text: &str) -> bool {
        // Check if the text starts with or contains any known secret pattern
        let lower = text.to_lowercase();
        SECRET_PATTERNS
            .iter()
            .any(|pattern| lower.contains(&pattern.to_lowercase()))
    }
}

impl SecretMaskerImpl {
    /// Generate a safe hint for a secret (reveals only first few chars).
    fn hint(secret: &str) -> String {
        if secret.len() > 8 {
            format!("{}...", &secret[..8])
        } else if secret.len() > 4 {
            format!("{}...", &secret[..4])
        } else {
            "****".to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mask_empty_secret() {
        let masker = SecretMaskerImpl::with_ci(true);
        let result = masker.mask("").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_mask_non_empty() {
        let masker = SecretMaskerImpl::with_ci(true);
        let result = masker.mask("my-secret-key-12345").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_mask_all() {
        let masker = SecretMaskerImpl::with_ci(true);
        let input = MaskSecretsInput {
            secrets: vec![
                "api-key-1".to_string(),
                "api-key-2".to_string(),
                "".to_string(), // empty should be skipped
            ],
        };
        let result = masker.mask_all(input).await.unwrap();
        assert_eq!(result.masked_count, 2);
        assert_eq!(result.masked_hints.len(), 2);
    }

    #[tokio::test]
    async fn test_contains_secret_openai_key() {
        let masker = SecretMaskerImpl::new();
        assert!(masker.contains_secret("sk-proj-abcdef123456").await);
    }

    #[tokio::test]
    async fn test_contains_secret_github_token() {
        let masker = SecretMaskerImpl::new();
        assert!(masker.contains_secret("ghp_abcdefghijklmnop").await);
    }

    #[tokio::test]
    async fn test_contains_secret_aws_key() {
        let masker = SecretMaskerImpl::new();
        assert!(masker.contains_secret("AKIAIOSFODNN7EXAMPLE").await);
    }

    #[tokio::test]
    async fn test_contains_secret_negative() {
        let masker = SecretMaskerImpl::new();
        assert!(!masker.contains_secret("hello-world-normal-text").await);
    }

    #[tokio::test]
    async fn test_contains_secret_case_insensitive() {
        let masker = SecretMaskerImpl::new();
        assert!(masker.contains_secret("SK-PROJ-TEST").await);
    }

    #[tokio::test]
    async fn test_contains_secret_empty() {
        let masker = SecretMaskerImpl::new();
        assert!(!masker.contains_secret("").await);
    }

    #[tokio::test]
    async fn test_contains_secret_slack_token() {
        let masker = SecretMaskerImpl::new();
        assert!(masker.contains_secret("xoxb-1234567890").await);
    }

    #[tokio::test]
    async fn test_hint_long_secret() {
        assert_eq!(SecretMaskerImpl::hint("my-secret-key-12345"), "my-secre...");
    }

    #[tokio::test]
    async fn test_hint_short_secret() {
        assert_eq!(SecretMaskerImpl::hint("abc"), "****");
    }

    #[tokio::test]
    async fn test_hint_medium_secret() {
        assert_eq!(SecretMaskerImpl::hint("abcdefg"), "abcd...");
    }

    #[tokio::test]
    async fn test_mask_all_empty_input() {
        let masker = SecretMaskerImpl::with_ci(true);
        let input = MaskSecretsInput { secrets: vec![] };
        let result = masker.mask_all(input).await.unwrap();
        assert_eq!(result.masked_count, 0);
        assert!(result.masked_hints.is_empty());
    }
}
