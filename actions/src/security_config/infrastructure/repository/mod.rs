//! Repository interfaces for the Security Configuration bounded context.
//!
//! @canonical actions/.pi/architecture/modules/security-config.md
//! Implements: Contract Freeze — ForkRepository, TokenRepository, PolicyRepository,
//! HmacKeyRepository, AllowlistRepository traits
//! Issue: issue-contract-freeze
//!
//! Repositories abstract data access behind interfaces, allowing
//! implementations to use environment variables, filesystem, GitHub API,
//! or mock storage without coupling domain logic to infrastructure.
//!
//! # Contract (Frozen)
//! - All repository methods are async
//! - All methods return domain error types
//! - No framework-specific annotations on trait definitions
//! - Implementations are hidden behind these interfaces

use async_trait::async_trait;

use crate::security_config::domain::{HmacKey, SecurityError, SecurityPolicy};

/// Repository for reading GitHub environment variables related to fork detection.
///
/// Abstracts `std::env::var` behind a trait for testability.
/// Implementations can use real environment variables or an in-memory map.
#[async_trait]
pub trait ForkRepository: Send + Sync {
    /// Get the base repository full name (e.g., "org/repo").
    /// Reads `GITHUB_REPOSITORY` env var.
    async fn base_repo(&self) -> Result<String, SecurityError>;

    /// Get the head repository full name for a PR.
    /// Reads `GITHUB_EVENT_PULL_REQUEST_HEAD_REPO_FULL_NAME` env var.
    async fn head_repo(&self) -> Result<Option<String>, SecurityError>;

    /// Get the head repository owner for a fork PR.
    /// Reads `GITHUB_EVENT_PULL_REQUEST_HEAD_REPO_OWNER` env var.
    async fn head_repo_owner(&self) -> Result<Option<String>, SecurityError>;

    /// Get the event name (e.g., "pull_request", "push").
    /// Reads `GITHUB_EVENT_NAME` env var.
    async fn event_name(&self) -> Result<Option<String>, SecurityError>;

    /// Get the PR number if this is a pull request event.
    /// Reads `GITHUB_EVENT_PULL_REQUEST_NUMBER` env var.
    async fn pr_number(&self) -> Result<Option<u64>, SecurityError>;
}

/// Repository for validating GitHub tokens and checking permissions.
///
/// Abstracts GitHub API calls behind a trait for testability.
#[async_trait]
pub trait TokenRepository: Send + Sync {
    /// Validate a GitHub token by calling the API.
    /// Returns true if the token is valid and responds.
    async fn validate_token(&self, token: &str) -> Result<bool, SecurityError>;

    /// Get the authenticated user's scopes/permissions.
    /// Returns a list of permission strings (e.g., "repo", "workflow").
    async fn get_scopes(&self, token: &str) -> Result<Vec<String>, SecurityError>;

    /// Check if the token has a specific scope.
    async fn has_scope(&self, token: &str, scope: &str) -> Result<bool, SecurityError>;
}

/// Repository for reading security policy configuration.
///
/// Abstracts filesystem and HTTP access to `.rigorix/security.toml` and
/// org-level policy URLs behind a trait for testability.
#[async_trait]
pub trait PolicyRepository: Send + Sync {
    /// Read the security policy file content.
    /// Searches CWD for `.rigorix/security.toml` by default.
    async fn read_policy_file(
        &self,
        path_override: Option<&str>,
    ) -> Result<Option<String>, SecurityError>;

    /// Fetch an organization-level policy from a URL.
    async fn fetch_org_policy(&self, url: &str) -> Result<String, SecurityError>;

    /// Resolve the path to the security policy file.
    async fn resolve_policy_path(&self) -> Result<Option<String>, SecurityError>;

    /// Check if the policy file exists.
    async fn policy_file_exists(&self) -> Result<bool, SecurityError>;
}

/// Repository for managing HMAC signing keys.
///
/// Abstracts key storage behind a trait for testability.
/// In production, keys are loaded from environment variables.
#[async_trait]
pub trait HmacKeyRepository: Send + Sync {
    /// Load the HMAC signing key from its configured source.
    /// Typically reads from an environment variable.
    async fn load_key(&self, env_var: &str) -> Result<Option<HmacKey>, SecurityError>;

    /// Store a new HMAC key (for key rotation).
    /// In production, this would update a secret store.
    async fn store_key(&self, key: &HmacKey) -> Result<(), SecurityError>;

    /// Get the key rotation configuration.
    async fn rotation_config(&self) -> Result<(u32, String), SecurityError>;

    /// List all available key IDs (for key rotation).
    async fn list_keys(&self) -> Result<Vec<String>, SecurityError>;
}

/// Repository for managing the URL allowlist.
///
/// Abstracts allowlist storage behind a trait for testability.
/// In production, the allowlist is loaded from the security policy.
#[async_trait]
pub trait AllowlistRepository: Send + Sync {
    /// Load the allowlist entries from configuration.
    async fn load_allowlist(&self) -> Result<Vec<String>, SecurityError>;

    /// Add an entry to the runtime allowlist.
    async fn add_entry(&self, host: String) -> Result<(), SecurityError>;

    /// Remove an entry from the runtime allowlist.
    async fn remove_entry(&self, host: &str) -> Result<(), SecurityError>;

    /// Check if a host is in the allowlist.
    async fn contains(&self, host: &str) -> Result<bool, SecurityError>;
}
