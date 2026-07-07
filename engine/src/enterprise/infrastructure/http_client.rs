//! HTTP client for the Rigorix Enterprise API.
//!
//! Handles fetching policy bundles with proper authentication.

use crate::enterprise::domain::{EnterpriseConfig, EnterpriseError, PolicyBundle};

/// HTTP client for communicating with the Rigorix Enterprise API.
///
/// Wraps a `reqwest::Client` with the enterprise API key for
/// authenticated requests.
#[derive(Debug, Clone)]
pub struct HttpEnterpriseClient {
    client: reqwest::Client,
}

impl HttpEnterpriseClient {
    /// Create a new enterprise HTTP client.
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        Self { client }
    }

    /// Create a new enterprise HTTP client with a custom timeout.
    pub fn with_timeout(timeout_secs: u64) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(timeout_secs))
            .build()
            .expect("Failed to create HTTP client");

        Self { client }
    }

    /// Fetch the policy bundle from the enterprise API.
    ///
    /// Sends a GET request to `{api_url}/policies/bundle?team_id={team_id}`
    /// with `Authorization: Bearer {api_key}`.
    pub async fn fetch_bundle(
        &self,
        config: &EnterpriseConfig,
    ) -> Result<PolicyBundle, EnterpriseError> {
        let url = format!(
            "{}/policies/bundle?team_id={}",
            config.api_url.trim_end_matches('/'),
            config.team_id
        );

        let response = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", config.api_key))
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    EnterpriseError::RequestFailed {
                        detail: format!("Request timed out: {e}"),
                    }
                } else if e.is_connect() {
                    EnterpriseError::RequestFailed {
                        detail: format!("Connection failed: {e}"),
                    }
                } else {
                    EnterpriseError::RequestFailed {
                        detail: e.to_string(),
                    }
                }
            })?;

        let status = response.status();
        match status.as_u16() {
            200 => {
                let bundle: PolicyBundle = response.json().await.map_err(|e| {
                    EnterpriseError::RequestFailed {
                        detail: format!("Failed to parse bundle response: {e}"),
                    }
                })?;
                Ok(bundle)
            }
            401 | 403 => Err(EnterpriseError::Unauthorized {
                detail: format!("HTTP {}", status),
            }),
            500..=599 => Err(EnterpriseError::ServerError {
                detail: format!("HTTP {}", status),
            }),
            other => Err(EnterpriseError::RequestFailed {
                detail: format!("Unexpected HTTP status: {other}"),
            }),
        }
    }
}

impl Default for HttpEnterpriseClient {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{header, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn sample_config(api_url: String) -> EnterpriseConfig {
        EnterpriseConfig {
            api_url,
            api_key: "test-api-key".to_string(),
            team_id: uuid::Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap(),
            fetch_policies: true,
            enforce_policies: true,
            post_audit: true,
            policy_cache_ttl_secs: 300,
        }
    }

    #[tokio::test]
    async fn test_fetch_bundle_success() {
        let mock_server = MockServer::start().await;

        let bundle_json = serde_json::json!({
            "team_id": "00000000-0000-0000-0000-000000000001",
            "generated_at": "2026-07-07T12:00:00Z",
            "policies": [
                {
                    "id": "11111111-1111-1111-1111-111111111111",
                    "name": "block-bash",
                    "rule_type": "tool_blocklist",
                    "rule_config": {"tools": ["bash"]},
                    "enforcement_mode": "enforce",
                    "severity": "critical",
                    "enabled": true
                }
            ],
            "signature": "sha256=abc123"
        });

        Mock::given(method("GET"))
            .and(path("/api/v1/policies/bundle"))
            .and(query_param("team_id", "00000000-0000-0000-0000-000000000001"))
            .and(header("Authorization", "Bearer test-api-key"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&bundle_json))
            .mount(&mock_server)
            .await;

        let client = HttpEnterpriseClient::new();
        let config = sample_config(format!("{}/api/v1", mock_server.uri()));
        let result = client.fetch_bundle(&config).await;

        assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
        let bundle = result.unwrap();
        assert_eq!(bundle.policies.len(), 1);
        assert_eq!(bundle.policies[0].name, "block-bash");
    }

    #[tokio::test]
    async fn test_fetch_bundle_unauthorized() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/api/v1/policies/bundle"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&mock_server)
            .await;

        let client = HttpEnterpriseClient::new();
        let config = sample_config(format!("{}/api/v1", mock_server.uri()));
        let result = client.fetch_bundle(&config).await;

        assert!(result.is_err());
        match result.unwrap_err() {
            EnterpriseError::Unauthorized { .. } => {} // expected
            other => panic!("Expected Unauthorized, got: {other}"),
        }
    }

    #[tokio::test]
    async fn test_fetch_bundle_server_error() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/api/v1/policies/bundle"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&mock_server)
            .await;

        let client = HttpEnterpriseClient::new();
        let config = sample_config(format!("{}/api/v1", mock_server.uri()));
        let result = client.fetch_bundle(&config).await;

        assert!(result.is_err());
        match result.unwrap_err() {
            EnterpriseError::ServerError { .. } => {} // expected
            other => panic!("Expected ServerError, got: {other}"),
        }
    }

    #[tokio::test]
    async fn test_fetch_bundle_unexpected_status() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/api/v1/policies/bundle"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&mock_server)
            .await;

        let client = HttpEnterpriseClient::new();
        let config = sample_config(format!("{}/api/v1", mock_server.uri()));
        let result = client.fetch_bundle(&config).await;

        assert!(result.is_err());
        match result.unwrap_err() {
            EnterpriseError::RequestFailed { detail } => {
                assert!(detail.contains("404"), "Expected 404 in detail, got: {detail}");
            }
            other => panic!("Expected RequestFailed, got: {other}"),
        }
    }

    #[tokio::test]
    async fn test_fetch_bundle_parse_error() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/api/v1/policies/bundle"))
            .respond_with(ResponseTemplate::new(200).set_body_string("not-json"))
            .mount(&mock_server)
            .await;

        let client = HttpEnterpriseClient::new();
        let config = sample_config(format!("{}/api/v1", mock_server.uri()));
        let result = client.fetch_bundle(&config).await;

        assert!(result.is_err());
        match result.unwrap_err() {
            EnterpriseError::RequestFailed { detail } => {
                assert!(
                    detail.contains("parse"),
                    "Expected parse error in detail, got: {detail}"
                );
            }
            other => panic!("Expected RequestFailed, got: {other}"),
        }
    }

    #[tokio::test]
    async fn test_fetch_bundle_forbidden() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/api/v1/policies/bundle"))
            .respond_with(ResponseTemplate::new(403))
            .mount(&mock_server)
            .await;

        let client = HttpEnterpriseClient::new();
        let config = sample_config(format!("{}/api/v1", mock_server.uri()));
        let result = client.fetch_bundle(&config).await;

        assert!(result.is_err());
        match result.unwrap_err() {
            EnterpriseError::Unauthorized { .. } => {} // 403 maps to Unauthorized
            other => panic!("Expected Unauthorized, got: {other}"),
        }
    }
}
