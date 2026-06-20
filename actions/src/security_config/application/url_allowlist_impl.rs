//! Implementation of `UrlAllowlistService`.
//!
//! @canonical actions/.pi/architecture/modules/security-config.md#url
//! Implements: UrlAllowlistService trait — validates URLs against allowlist
//! Issue: #542
//!
//! Validates backend URLs against a configured allowlist loaded from
//! `.rigorix/security.toml`. Prevents exfiltration of audit data.

use async_trait::async_trait;

use crate::security_config::application::dto::{ValidateUrlInput, ValidateUrlOutput};
use crate::security_config::application::service::UrlAllowlistService;
use crate::security_config::domain::SecurityError;

/// Implementation of `UrlAllowlistService`.
///
/// Validates URLs by parsing them and checking host against allowlist.
/// If no allowlist is configured, all URLs are allowed (fail-open for dev).
pub struct UrlAllowlistImpl {
    /// Known allowed hosts.
    allowlist: Vec<String>,
}

impl UrlAllowlistImpl {
    pub fn new(allowlist: Vec<String>) -> Self {
        Self {
            allowlist: allowlist.into_iter().map(|h| h.to_lowercase()).collect(),
        }
    }

    /// Create from a comma-separated string of hosts.
    pub fn from_csv(csv: &str) -> Self {
        let hosts: Vec<String> = csv
            .split(',')
            .map(|s| s.trim().to_lowercase())
            .filter(|s| !s.is_empty())
            .collect();
        Self::new(hosts)
    }

    /// Extract the host portion from a URL string without using a URL parser crate.
    fn extract_host(url: &str) -> Option<String> {
        // Strip protocol prefix
        let after_protocol = if let Some(pos) = url.find("://") {
            &url[pos + 3..]
        } else {
            url
        };

        // Take everything before the first '/' or '?' or '#'
        let host_part = after_protocol.split(['/', '?', '#']).next().unwrap_or("");

        // Remove user:password@ prefix if present
        let host_only = if let Some(at_pos) = host_part.rfind('@') {
            &host_part[at_pos + 1..]
        } else {
            host_part
        };

        // Remove port number if present
        let host_without_port = host_only.split(':').next().unwrap_or("");

        if host_without_port.is_empty() {
            None
        } else {
            Some(host_without_port.to_string())
        }
    }
}

impl Default for UrlAllowlistImpl {
    fn default() -> Self {
        Self::new(vec![])
    }
}

#[async_trait]
impl UrlAllowlistService for UrlAllowlistImpl {
    async fn validate(&self, input: ValidateUrlInput) -> Result<ValidateUrlOutput, SecurityError> {
        let allowlist = input
            .allowlist_override
            .unwrap_or_else(|| self.allowlist.clone());

        if allowlist.is_empty() {
            // No allowlist configured — allow all (development mode)
            return Ok(ValidateUrlOutput {
                allowed: true,
                host: "unknown".to_string(),
                checked_against: vec![],
            });
        }

        // Simple validation: URL must have a protocol prefix
        if !input.url.contains("://") {
            return Err(SecurityError::InvalidUrl(input.url.clone()));
        }

        let host = Self::extract_host(&input.url)
            .ok_or_else(|| SecurityError::InvalidUrl(input.url.clone()))?
            .to_lowercase();
        let allowed = allowlist
            .iter()
            .any(|a| host == *a || host.ends_with(&format!(".{}", a)));

        if !allowed {
            return Err(SecurityError::UrlBlocked {
                url: input.url.clone(),
                allowed: allowlist.clone(),
            });
        }

        Ok(ValidateUrlOutput {
            allowed: true,
            host,
            checked_against: allowlist,
        })
    }

    async fn load_allowlist(&self) -> Result<Vec<String>, SecurityError> {
        Ok(self.allowlist.clone())
    }

    async fn add_allowed_host(&self, host: String) -> Result<(), SecurityError> {
        // Note: immutable by design; use AllowlistRepository for mutations
        tracing::warn!(
            "Runtime allowlist addition requested but not supported: {}",
            host
        );
        Ok(())
    }

    async fn is_host_allowed(&self, host: &str) -> Result<bool, SecurityError> {
        let host_lower = host.to_lowercase();
        Ok(self
            .allowlist
            .iter()
            .any(|a| host_lower == *a || host_lower.ends_with(&format!(".{}", a))))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_allowlist(hosts: Vec<&str>) -> UrlAllowlistImpl {
        UrlAllowlistImpl::new(hosts.into_iter().map(String::from).collect())
    }

    #[tokio::test]
    async fn test_validate_allowed_url() {
        let allowlist = make_allowlist(vec!["api.rigorix.io"]);
        let input = ValidateUrlInput {
            url: "https://api.rigorix.io/v1/audit".to_string(),
            allowlist_override: None,
        };
        let result = allowlist.validate(input).await.unwrap();
        assert!(result.allowed);
    }

    #[tokio::test]
    async fn test_validate_blocked_url() {
        let allowlist = make_allowlist(vec!["api.rigorix.io"]);
        let input = ValidateUrlInput {
            url: "https://evil.com/exfiltrate".to_string(),
            allowlist_override: None,
        };
        let result = allowlist.validate(input).await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            SecurityError::UrlBlocked { .. }
        ));
    }

    #[tokio::test]
    async fn test_validate_empty_allowlist_allows_all() {
        let allowlist = make_allowlist(vec![]);
        let input = ValidateUrlInput {
            url: "https://anything.com/path".to_string(),
            allowlist_override: None,
        };
        let result = allowlist.validate(input).await.unwrap();
        assert!(result.allowed);
    }

    #[tokio::test]
    async fn test_validate_subdomain_match() {
        let allowlist = make_allowlist(vec!["rigorix.io"]);
        let input = ValidateUrlInput {
            url: "https://api.rigorix.io/endpoint".to_string(),
            allowlist_override: None,
        };
        let result = allowlist.validate(input).await.unwrap();
        assert!(result.allowed);
    }

    #[tokio::test]
    async fn test_validate_invalid_url() {
        let allowlist = make_allowlist(vec!["rigorix.io"]);
        let input = ValidateUrlInput {
            url: "not-a-valid-url".to_string(),
            allowlist_override: None,
        };
        let result = allowlist.validate(input).await;
        assert!(matches!(result.unwrap_err(), SecurityError::InvalidUrl(_)));
    }

    #[tokio::test]
    async fn test_validate_with_override() {
        let allowlist = make_allowlist(vec![]); // empty by default
        let input = ValidateUrlInput {
            url: "https://api.rigorix.io".to_string(),
            allowlist_override: Some(vec!["api.rigorix.io".to_string()]),
        };
        let result = allowlist.validate(input).await.unwrap();
        assert!(result.allowed);
    }

    #[tokio::test]
    async fn test_is_host_allowed() {
        let allowlist = make_allowlist(vec!["rigorix.io", "github.com"]);
        assert!(allowlist.is_host_allowed("rigorix.io").await.unwrap());
        assert!(allowlist.is_host_allowed("api.rigorix.io").await.unwrap());
        assert!(!allowlist.is_host_allowed("evil.com").await.unwrap());
    }

    #[tokio::test]
    async fn test_from_csv() {
        let allowlist = UrlAllowlistImpl::from_csv("rigorix.io,github.com,api.test.com");
        assert!(allowlist.is_host_allowed("rigorix.io").await.unwrap());
        assert!(allowlist.is_host_allowed("github.com").await.unwrap());
        assert!(!allowlist.is_host_allowed("other.com").await.unwrap());
    }

    #[tokio::test]
    async fn test_case_insensitive_match() {
        let allowlist = make_allowlist(vec!["Api.Rigorix.Io"]);
        let input = ValidateUrlInput {
            url: "https://api.rigorix.io".to_string(),
            allowlist_override: None,
        };
        let result = allowlist.validate(input).await.unwrap();
        assert!(result.allowed);
    }
}
