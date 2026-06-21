//! GitHub REST API client — shared HTTP wrapper with rate limiting.
//!
//! @canonical actions/.pi/architecture/modules/ci-integration.md#client
//! Issue: Phase 1.2 scaffold
//!
//! Provides a lightweight wrapper around the GitHub REST API used by
//! all action modules. Handles authentication, rate limiting (respects
//! `X-RateLimit-Remaining` and `Retry-After` headers), and provides
//! typed methods for common GitHub operations.
//!
//! # Contract (Frozen)
//! - All methods return `Result<T, GitHubClientError>`
//! - Rate limiting is handled transparently inside the client
//! - Token is never logged (Debug impl masks it)
//! - Retry-After header is respected for 429 responses

use reqwest::header::{AUTHORIZATION, HeaderMap, HeaderValue, USER_AGENT};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::time::Duration;
use thiserror::Error;

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

#[derive(Debug, Error)]
pub enum GitHubClientError {
    #[error("GitHub API authentication failed: {0}")]
    AuthFailed(String),

    #[error("GitHub API rate limit exceeded (retry after {retry_after_secs}s)")]
    RateLimited { retry_after_secs: u64 },

    #[error("GitHub API resource not found: {0}")]
    NotFound(String),

    #[error("GitHub API permission denied: {0}")]
    PermissionDenied(String),

    #[error("GitHub API request failed ({status}): {message}")]
    ApiError { status: u16, message: String },

    #[error("Network error: {0}")]
    NetworkError(#[from] reqwest::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

// ---------------------------------------------------------------------------
// Data types
// ---------------------------------------------------------------------------

/// Commit status state as defined by the GitHub Statuses API.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StatusState {
    Pending,
    Success,
    Failure,
    Error,
}

/// Parameters for creating a commit status.
#[derive(Debug, Clone, Serialize)]
pub struct GitHubStatus {
    pub state: StatusState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_url: Option<String>,
    pub description: String,
    pub context: String,
}

/// A GitHub issue/PR comment.
#[derive(Debug, Clone, Deserialize)]
pub struct Comment {
    pub id: u64,
    pub body: String,
    #[serde(default, rename = "created_at")]
    pub created_at: String,
}

/// A GitHub issue label.
#[derive(Debug, Clone, Deserialize)]
pub struct Label {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub color: Option<String>,
}

// ---------------------------------------------------------------------------
// GitHubClient
// ---------------------------------------------------------------------------

/// Lightweight GitHub REST API client with rate limiting.
///
/// Used by `security-config`, `diff-analyzer`, `ci-integration`,
/// and `audit-posting`. Extracted here to avoid circular dependencies.
///
/// # Rate Limiting
///
/// - Respects `X-RateLimit-Remaining` header — logs warnings when low
/// - Respects `Retry-After` header on 429 responses — sleeps before returning
/// - Does NOT automatically retry (retry logic lives in calling modules)
///
/// # Authentication
///
/// Uses the `GITHUB_TOKEN` environment variable by default, or a
/// caller-provided token. The token is masked in Debug output.
pub struct GitHubClient {
    token: String,
    http: reqwest::Client,
    base_url: String,
}

impl GitHubClient {
    /// Default GitHub API base URL.
    pub const DEFAULT_API_URL: &'static str = "https://api.github.com";

    /// Create a new GitHubClient with the given token.
    pub fn new(token: impl Into<String>) -> Self {
        Self {
            token: token.into(),
            http: reqwest::Client::new(),
            base_url: Self::DEFAULT_API_URL.to_string(),
        }
    }

    /// Create a GitHubClient from the `GITHUB_TOKEN` environment variable.
    pub fn from_env() -> Result<Self, GitHubClientError> {
        let token = std::env::var("GITHUB_TOKEN")
            .map_err(|_| GitHubClientError::AuthFailed("GITHUB_TOKEN not set".into()))?;
        Ok(Self::new(token))
    }

    /// Set a custom base URL (for GitHub Enterprise Server).
    pub fn with_base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into();
        self
    }

    // ── HTTP helpers ──

    fn auth_headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", self.token)).expect("token is valid ASCII"),
        );
        headers.insert(USER_AGENT, HeaderValue::from_static("rigorix-action/0.1.0"));
        headers
    }

    async fn check_rate_limit(
        &self,
        response: &reqwest::Response,
    ) -> Result<(), GitHubClientError> {
        if response.status() == 429 {
            let retry_after = response
                .headers()
                .get("Retry-After")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(60);

            tracing::warn!(
                retry_after_secs = retry_after,
                "GitHub API rate limit exceeded"
            );

            tokio::time::sleep(Duration::from_secs(retry_after)).await;
            return Err(GitHubClientError::RateLimited {
                retry_after_secs: retry_after,
            });
        }

        let remaining = response
            .headers()
            .get("X-RateLimit-Remaining")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u32>().ok());

        if let Some(rem) = remaining
            && rem < 10
        {
            tracing::warn!(remaining = rem, "GitHub API rate limit running low");
        }

        Ok(())
    }

    async fn handle_response(
        &self,
        response: reqwest::Response,
    ) -> Result<String, GitHubClientError> {
        self.check_rate_limit(&response).await?;

        let status = response.status();
        if status.is_success() {
            response
                .text()
                .await
                .map_err(GitHubClientError::NetworkError)
        } else if status.as_u16() == 401 {
            Err(GitHubClientError::AuthFailed(
                "Invalid or expired token".into(),
            ))
        } else if status.as_u16() == 403 {
            Err(GitHubClientError::PermissionDenied(
                "HTTP 403: Insufficient permissions".to_string(),
            ))
        } else if status.as_u16() == 404 {
            Err(GitHubClientError::NotFound(
                "Resource not found".to_string(),
            ))
        } else {
            let body = response.text().await.unwrap_or_default();
            Err(GitHubClientError::ApiError {
                status: status.as_u16(),
                message: body,
            })
        }
    }

    // ── API methods ──

    /// Fetch a pull request diff as raw text.
    ///
    /// Uses the `application/vnd.github.v3.diff` media type.
    pub async fn fetch_pr_diff(&self, repo: &str, pr: u64) -> Result<String, GitHubClientError> {
        let url = format!("{}/repos/{}/pulls/{}", self.base_url, repo, pr);

        let response = self
            .http
            .get(&url)
            .headers(self.auth_headers())
            .header("Accept", "application/vnd.github.v3.diff")
            .send()
            .await?;

        self.handle_response(response).await
    }

    /// Create a commit status.
    pub async fn create_status(
        &self,
        repo: &str,
        sha: &str,
        status: &GitHubStatus,
    ) -> Result<(), GitHubClientError> {
        let url = format!("{}/repos/{}/statuses/{}", self.base_url, repo, sha);

        let response = self
            .http
            .post(&url)
            .headers(self.auth_headers())
            .json(status)
            .send()
            .await?;

        self.handle_response(response).await?;
        Ok(())
    }

    /// Create an issue comment (works for both issues and PRs).
    pub async fn create_issue_comment(
        &self,
        repo: &str,
        issue: u64,
        body: &str,
    ) -> Result<Comment, GitHubClientError> {
        let url = format!("{}/repos/{}/issues/{}/comments", self.base_url, repo, issue);

        let response = self
            .http
            .post(&url)
            .headers(self.auth_headers())
            .json(&serde_json::json!({ "body": body }))
            .send()
            .await?;

        let text = self.handle_response(response).await?;
        serde_json::from_str(&text).map_err(GitHubClientError::Serialization)
    }

    /// Update an existing issue comment.
    pub async fn update_issue_comment(
        &self,
        comment_id: u64,
        body: &str,
    ) -> Result<(), GitHubClientError> {
        let url = format!("{}/repos/issues/comments/{}", self.base_url, comment_id);

        let response = self
            .http
            .patch(&url)
            .headers(self.auth_headers())
            .json(&serde_json::json!({ "body": body }))
            .send()
            .await?;

        self.handle_response(response).await?;
        Ok(())
    }

    /// List comments on an issue or PR.
    pub async fn list_issue_comments(
        &self,
        repo: &str,
        issue: u64,
    ) -> Result<Vec<Comment>, GitHubClientError> {
        let url = format!("{}/repos/{}/issues/{}/comments", self.base_url, repo, issue);

        let response = self
            .http
            .get(&url)
            .headers(self.auth_headers())
            .send()
            .await?;

        let text = self.handle_response(response).await?;
        serde_json::from_str(&text).map_err(GitHubClientError::Serialization)
    }

    /// Add labels to an issue or PR.
    pub async fn add_labels(
        &self,
        repo: &str,
        issue: u64,
        labels: &[&str],
    ) -> Result<(), GitHubClientError> {
        let url = format!("{}/repos/{}/issues/{}/labels", self.base_url, repo, issue);

        let response = self
            .http
            .post(&url)
            .headers(self.auth_headers())
            .json(&serde_json::json!({ "labels": labels }))
            .send()
            .await?;

        self.handle_response(response).await?;
        Ok(())
    }

    /// Remove a label from an issue or PR.
    pub async fn remove_label(
        &self,
        repo: &str,
        issue: u64,
        label: &str,
    ) -> Result<(), GitHubClientError> {
        let url = format!(
            "{}/repos/{}/issues/{}/labels/{}",
            self.base_url,
            repo,
            issue,
            urlencoding(label)
        );

        let response = self
            .http
            .delete(&url)
            .headers(self.auth_headers())
            .send()
            .await?;

        self.handle_response(response).await?;
        Ok(())
    }

    /// Read a file from a specific git ref (branch, tag, commit SHA).
    ///
    /// Used by PolicyLoader to read policy files from the base branch.
    pub async fn read_file_from_ref(
        &self,
        repo: &str,
        path: &str,
        ref_name: &str,
    ) -> Result<String, GitHubClientError> {
        let url = format!(
            "{}/repos/{}/contents/{}?ref={}",
            self.base_url, repo, path, ref_name
        );

        let response = self
            .http
            .get(&url)
            .headers(self.auth_headers())
            .header("Accept", "application/vnd.github.v3.raw")
            .send()
            .await?;

        self.handle_response(response).await
    }

    /// Validate that the token is valid by calling the `/user` endpoint.
    pub async fn validate_token(&self) -> Result<bool, GitHubClientError> {
        let url = format!("{}/user", self.base_url);

        let response = self
            .http
            .get(&url)
            .headers(self.auth_headers())
            .send()
            .await?;

        Ok(response.status().is_success())
    }
}

impl fmt::Debug for GitHubClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GitHubClient")
            .field("token", &"[MASKED]")
            .field("base_url", &self.base_url)
            .finish()
    }
}

impl Clone for GitHubClient {
    fn clone(&self) -> Self {
        Self {
            token: self.token.clone(),
            http: reqwest::Client::new(),
            base_url: self.base_url.clone(),
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// URL-encode a label name for the GitHub API.
fn urlencoding(s: &str) -> String {
    s.replace(' ', "%20")
        .replace('#', "%23")
        .replace('/', "%2F")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_debug_masks_token() {
        let client = GitHubClient::new("ghp_secret12345");
        let debug = format!("{:?}", client);
        assert!(!debug.contains("ghp_secret12345"));
        assert!(debug.contains("[MASKED]"));
    }

    #[test]
    fn test_client_from_env_missing() {
        // SAFETY: test-only environment manipulation — no concurrent access
        unsafe { std::env::remove_var("GITHUB_TOKEN") };
        let result = GitHubClient::from_env();
        assert!(result.is_err());
    }

    #[test]
    fn test_status_state_serialization() {
        assert_eq!(
            serde_json::to_string(&StatusState::Success).unwrap(),
            "\"success\""
        );
        assert_eq!(
            serde_json::to_string(&StatusState::Failure).unwrap(),
            "\"failure\""
        );
    }

    #[test]
    fn test_urlencoding_label() {
        assert_eq!(urlencoding("rigorix:needs-fix"), "rigorix:needs-fix");
        assert_eq!(urlencoding("bug fix"), "bug%20fix");
    }

    #[test]
    fn test_client_clone() {
        let client = GitHubClient::new("token").with_base_url("https://github.example.com");
        let cloned = client.clone();
        assert_eq!(format!("{:?}", cloned), format!("{:?}", client));
    }
}
