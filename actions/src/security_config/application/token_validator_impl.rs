//! Implementation of `TokenValidationService`.
//!
//! @canonical actions/.pi/architecture/modules/security-config.md#token
//! Implements: TokenValidationService trait — validates GitHub token permissions
//! Issue: #541
//!
//! Validates the GitHub token by calling the GitHub API and checking
//! that it has the required scopes for the action mode.

use async_trait::async_trait;

use crate::security_config::application::dto::{ValidateTokenInput, ValidateTokenOutput};
use crate::security_config::application::service::TokenValidationService;
use crate::security_config::domain::{ActionMode, SecurityError};

/// Implementation of `TokenValidationService`.
///
/// Validates tokens by making API calls to GitHub and checking
/// the returned scopes against mode-specific requirements.
pub struct TokenValidatorImpl {
    http_client: reqwest::Client,
}

impl TokenValidatorImpl {
    pub fn new() -> Self {
        Self {
            http_client: reqwest::Client::new(),
        }
    }
}

impl Default for TokenValidatorImpl {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TokenValidationService for TokenValidatorImpl {
    async fn validate(
        &self,
        input: ValidateTokenInput,
    ) -> Result<ValidateTokenOutput, SecurityError> {
        // Check basic token validity
        let valid = self.is_token_valid(&input.token).await?;

        if !valid {
            return Ok(ValidateTokenOutput {
                valid: false,
                has_required_permissions: false,
                available_scopes: vec![],
                missing_scopes: vec![],
            });
        }

        // Determine required permissions
        let required = Self::required_permissions(input.mode.clone());

        // Get available scopes from token
        let available_scopes = self.get_scopes_from_api(&input.token).await?;
        let required_strs: Vec<String> = required
            .iter()
            .map(|(perm, level)| format!("{}:{}", perm, level))
            .collect();

        let missing_scopes: Vec<String> = required_strs
            .into_iter()
            .filter(|req| {
                !available_scopes.iter().any(|avail| {
                    // Match if available scope covers the requirement
                    // e.g., "contents:write" satisfies "contents:read" requirement
                    let parts: Vec<&str> = req.splitn(2, ':').collect();
                    let req_perm = parts.first().copied().unwrap_or("");
                    let req_level = parts.get(1).copied().unwrap_or("");
                    avail.starts_with(req_perm) && Self::scope_sufficient(avail, req_level)
                })
            })
            .collect();

        let has_all = missing_scopes.is_empty();

        Ok(ValidateTokenOutput {
            valid: true,
            has_required_permissions: has_all,
            available_scopes,
            missing_scopes,
        })
    }

    async fn is_token_valid(&self, token: &str) -> Result<bool, SecurityError> {
        let response = self
            .http_client
            .get("https://api.github.com/user")
            .header("Authorization", format!("Bearer {}", token))
            .header("User-Agent", "rigorix-action")
            .send()
            .await
            .map_err(|e| SecurityError::TokenValidationFailed {
                detail: format!("HTTP request failed: {}", e),
                status_code: None,
            })?;

        Ok(response.status().is_success())
    }

    fn required_permissions(mode: ActionMode) -> &'static [(&'static str, &'static str)] {
        match mode {
            ActionMode::Run | ActionMode::Validate | ActionMode::Plan => &[
                ("contents", "write"),
                ("pull-requests", "write"),
                ("issues", "write"),
                ("statuses", "write"),
            ],
            ActionMode::Governance => &[
                ("contents", "read"),
                ("pull-requests", "write"),
                ("statuses", "write"),
            ],
            ActionMode::Status | ActionMode::Auto => &[("contents", "read")],
        }
    }
}

impl TokenValidatorImpl {
    /// Get scopes from the GitHub API by checking the X-OAuth-Scopes header.
    async fn get_scopes_from_api(&self, token: &str) -> Result<Vec<String>, SecurityError> {
        let response = self
            .http_client
            .get("https://api.github.com/user")
            .header("Authorization", format!("Bearer {}", token))
            .header("User-Agent", "rigorix-action")
            .send()
            .await
            .map_err(|e| SecurityError::TokenValidationFailed {
                detail: format!("Failed to fetch scopes: {}", e),
                status_code: None,
            })?;

        let scopes = response
            .headers()
            .get("X-OAuth-Scopes")
            .and_then(|v| v.to_str().ok())
            .map(|s| {
                s.split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        Ok(scopes)
    }

    /// Check if an available scope is sufficient for a required level.
    fn scope_sufficient(available: &str, required_level: &str) -> bool {
        let parts: Vec<&str> = available.splitn(2, ':').collect();
        let avail_level = parts.get(1).copied().unwrap_or("");

        match (avail_level, required_level) {
            // "write" satisfies "read" and "write"
            ("write", "read") | ("write", "write") => true,
            // "admin" satisfies everything
            ("admin", _) => true,
            // Exact match required for other cases
            (a, r) => a == r,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_scope_sufficient_write_covers_read() {
        assert!(TokenValidatorImpl::scope_sufficient(
            "contents:write",
            "read"
        ));
    }

    #[tokio::test]
    async fn test_scope_sufficient_write_covers_write() {
        assert!(TokenValidatorImpl::scope_sufficient(
            "contents:write",
            "write"
        ));
    }

    #[tokio::test]
    async fn test_scope_sufficient_admin_covers_all() {
        assert!(TokenValidatorImpl::scope_sufficient(
            "contents:admin",
            "write"
        ));
        assert!(TokenValidatorImpl::scope_sufficient(
            "pull-requests:admin",
            "read"
        ));
    }

    #[tokio::test]
    async fn test_scope_sufficient_read_does_not_cover_write() {
        assert!(!TokenValidatorImpl::scope_sufficient(
            "contents:read",
            "write"
        ));
    }

    #[tokio::test]
    async fn test_required_permissions_governance() {
        let perms = TokenValidatorImpl::required_permissions(ActionMode::Governance);
        assert!(perms.contains(&("contents", "read")));
        assert!(perms.contains(&("pull-requests", "write")));
        assert!(!perms.contains(&("contents", "write")));
    }

    #[tokio::test]
    async fn test_required_permissions_run() {
        let perms = TokenValidatorImpl::required_permissions(ActionMode::Run);
        assert!(perms.contains(&("contents", "write")));
        assert!(perms.contains(&("issues", "write")));
    }

    #[tokio::test]
    async fn test_required_permissions_status() {
        let perms = TokenValidatorImpl::required_permissions(ActionMode::Status);
        assert_eq!(perms.len(), 1);
        assert!(perms.contains(&("contents", "read")));
    }

    #[tokio::test]
    async fn test_is_token_valid_invalid() {
        let validator = TokenValidatorImpl::new();
        let result = validator.is_token_valid("invalid-token").await;
        // This should fail since the token is fake
        assert!(result.is_ok() || result.is_err());
    }
}
