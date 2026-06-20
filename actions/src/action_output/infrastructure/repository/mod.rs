//! Repository interfaces for the Action Output bounded context.
//!
//! @canonical actions/.pi/architecture/modules/action-output.md
//! Implements: Contract Freeze — OutputRepository, SummaryRepository, GitHubApiClient traits
//! Issue: issue-contract-freeze
//!
//! Repositories abstract I/O behind interfaces, allowing implementations
//! to use environment variables, filesystem, or mock storage without
//! coupling domain logic to infrastructure.
//!
//! # Contract (Frozen)
//! - All repository methods are async
//! - All methods return domain error types
//! - No framework-specific annotations on trait definitions
//! - Implementations are hidden behind these interfaces

pub mod env_repository_impl;
pub mod github_api_client_impl;
pub mod output_repository_impl;

pub use env_repository_impl::*;
pub use github_api_client_impl::*;
pub use output_repository_impl::*;

use async_trait::async_trait;

use crate::action_output::domain::ActionOutputError;

/// Repository for writing output content to filesystem destinations.
///
/// Abstracts filesystem I/O for:
/// - Writing annotations to stdout (workflow commands)
/// - Writing output variables to `GITHUB_OUTPUT`
/// - Writing step summaries to `GITHUB_STEP_SUMMARY`
///
/// # Security
/// - File paths are validated against directory traversal
/// - Written content is sanitized (newlines stripped from variable values)
/// - Sensitive values (tokens) are never written via this repository
#[async_trait]
pub trait OutputRepository: Send + Sync {
    /// Write raw content to stdout.
    ///
    /// Used for emitting workflow command annotations.
    /// The GitHub Actions runner parses workflow commands from stdout.
    async fn write_stdout(&self, content: &str) -> Result<u64, ActionOutputError>;

    /// Write a `name=value` pair to the `GITHUB_OUTPUT` file.
    ///
    /// Appends to the file at the path specified by `GITHUB_OUTPUT`.
    async fn write_output_variable(
        &self,
        name: &str,
        value: &str,
    ) -> Result<u64, ActionOutputError>;

    /// Write markdown content to the `GITHUB_STEP_SUMMARY` file.
    ///
    /// Appends to the file at the path specified by `GITHUB_STEP_SUMMARY`.
    async fn append_summary(&self, markdown: &str) -> Result<u64, ActionOutputError>;

    /// Overwrite the `GITHUB_STEP_SUMMARY` file with new content.
    async fn overwrite_summary(&self, markdown: &str) -> Result<u64, ActionOutputError>;

    /// Get the path to the `GITHUB_OUTPUT` file.
    async fn get_output_path(&self) -> Result<Option<String>, ActionOutputError>;

    /// Get the path to the `GITHUB_STEP_SUMMARY` file.
    async fn get_summary_path(&self) -> Result<Option<String>, ActionOutputError>;

    /// Check if we're running in a GitHub Actions environment.
    async fn is_github_actions(&self) -> bool;
}

/// Repository for reading environment variables relevant to output formatting.
///
/// Abstracts `std::env::var` behind a trait for testability.
///
/// # Security
/// - Implementations MUST NOT log environment variable values
/// - Variable names (keys) are safe for logging
#[async_trait]
pub trait EnvRepository: Send + Sync {
    /// Read an environment variable by name.
    ///
    /// Returns `Ok(None)` if the variable is not set.
    async fn read_env_var(&self, name: &str) -> Result<Option<String>, ActionOutputError>;

    /// Read `GITHUB_STEP_SUMMARY` path.
    async fn read_step_summary_path(&self) -> Result<Option<String>, ActionOutputError>;

    /// Read `GITHUB_OUTPUT` path.
    async fn read_output_path(&self) -> Result<Option<String>, ActionOutputError>;

    /// Read `GITHUB_TOKEN` (or `INPUT_GITHUB_TOKEN`).
    async fn read_github_token(&self) -> Result<Option<String>, ActionOutputError>;

    /// Read `GITHUB_REPOSITORY` (format: `owner/repo`).
    async fn read_repository(&self) -> Result<Option<String>, ActionOutputError>;

    /// Read CI-related environment context.
    async fn read_ci_context(
        &self,
    ) -> Result<std::collections::HashMap<String, String>, ActionOutputError>;
}

/// Client for posting PR comments via the GitHub REST API.
///
/// Abstracts the GitHub API behind a trait for testability.
///
/// # Security
/// - Token is passed in the `Authorization` header
/// - Token is never logged or included in error messages
/// - HTTPS only
#[async_trait]
pub trait GitHubApiClient: Send + Sync {
    /// Post a comment to an issue or pull request.
    ///
    /// Endpoint: `POST /repos/{owner}/{repo}/issues/{issue_number}/comments`
    async fn post_comment(
        &self,
        repo: &str,
        issue_number: u64,
        body: &str,
        token: &str,
    ) -> Result<GitHubCommentResponse, ActionOutputError>;

    /// Check if the API is accessible with the given token.
    ///
    /// Makes a lightweight request to verify token validity.
    async fn health_check(&self, token: &str) -> Result<bool, ActionOutputError>;

    /// Get the authenticated user's login name.
    async fn get_authenticated_user(&self, token: &str) -> Result<String, ActionOutputError>;
}

/// Response from posting a GitHub comment.
#[derive(Debug, Clone)]
pub struct GitHubCommentResponse {
    /// The comment ID.
    pub id: u64,
    /// The HTML URL of the comment.
    pub html_url: String,
    /// The API URL of the comment.
    pub url: String,
}
