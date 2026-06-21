//! Repository interfaces for the CI Integration bounded context.
//!
//! @canonical actions/.pi/architecture/modules/ci-integration.md
//! Implements: Contract Freeze — StatusCheckRepository, PrCommentRepository traits
//! Issue: issue-contract-freeze
//!
//! Repositories abstract data access behind interfaces, allowing
//! implementations to use the GitHub REST API, environment variables,
//! filesystem, or mock storage without coupling domain logic to
//! infrastructure.
//!
//! # Contract (Frozen)
//! - All repository methods are async
//! - All methods return domain error types
//! - No framework-specific annotations on trait definitions
//! - Implementations are hidden behind these interfaces

pub mod status_check_repository_impl;

pub use status_check_repository_impl::*;

use async_trait::async_trait;
use uuid::Uuid;

use crate::ci_integration::domain::{CiIntegrationError, GitHubStatus, PrComment};

/// Repository for managing GitHub commit status checks.
///
/// Abstracts the GitHub REST API `POST /repos/{owner}/{repo}/statuses/{sha}`
/// endpoint behind a trait for testability.
///
/// # Security
/// - Commit SHA values must be validated before sending to GitHub API
/// - GitHub token is managed by the shared GitHubClient, not this repository
#[async_trait]
pub trait StatusCheckRepository: Send + Sync {
    /// Create or update a commit status check.
    ///
    /// Sends a status update to the GitHub API for the given commit SHA.
    /// The status replaces any existing status with the same context.
    async fn create_status(
        &self,
        owner: &str,
        repo: &str,
        sha: &str,
        status: GitHubStatus,
    ) -> Result<(), CiIntegrationError>;

    /// Get the current status check for a given context.
    ///
    /// Returns the latest status check with the matching context,
    /// or `None` if no status check exists with that context.
    async fn get_status(
        &self,
        owner: &str,
        repo: &str,
        sha: &str,
        context: &str,
    ) -> Result<Option<GitHubStatus>, CiIntegrationError>;

    /// List all status checks for a commit.
    ///
    /// Returns the combined status context for the commit.
    async fn list_statuses(
        &self,
        owner: &str,
        repo: &str,
        sha: &str,
    ) -> Result<Vec<GitHubStatus>, CiIntegrationError>;
}

/// Repository for managing GitHub issue/PR comments.
///
/// Abstracts the GitHub REST API comments endpoints behind a trait
/// for testability.
///
/// # Security
/// - Comment bodies must be sanitised to prevent injection attacks
/// - Bot identifiers must not be spoofable
#[async_trait]
pub trait PrCommentRepository: Send + Sync {
    /// Create a new comment on an issue or PR.
    async fn create_comment(
        &self,
        owner: &str,
        repo: &str,
        issue_number: u64,
        body: &str,
    ) -> Result<PrComment, CiIntegrationError>;

    /// Update an existing comment by ID.
    async fn update_comment(
        &self,
        owner: &str,
        repo: &str,
        comment_id: u64,
        body: &str,
    ) -> Result<PrComment, CiIntegrationError>;

    /// List all comments on an issue or PR.
    ///
    /// Returns comments ordered by creation date (ascending).
    async fn list_comments(
        &self,
        owner: &str,
        repo: &str,
        issue_number: u64,
    ) -> Result<Vec<PrComment>, CiIntegrationError>;

    /// Get a single comment by ID.
    async fn get_comment(
        &self,
        owner: &str,
        repo: &str,
        comment_id: u64,
    ) -> Result<PrComment, CiIntegrationError>;

    /// Delete a comment by ID.
    async fn delete_comment(
        &self,
        owner: &str,
        repo: &str,
        comment_id: u64,
    ) -> Result<(), CiIntegrationError>;
}

/// Repository for tracking execution history per PR.
///
/// Used for idempotency — prevents duplicate executions for the same
/// PR and intent within a configurable window.
///
/// # Contract (Frozen)
/// - `record_execution()` stores an execution for a PR
/// - `is_duplicate()` checks if an execution was already recorded
/// - `cleanup()` removes stale entries beyond the retention window
#[async_trait]
pub trait ExecutionTrackerRepository: Send + Sync {
    /// Record an execution for the given PR.
    ///
    /// Returns `Ok(true)` if recorded successfully.
    /// Returns `Err(DuplicateExecution)` if the same execution was already recorded.
    async fn record_execution(
        &self,
        pr_number: u64,
        execution_id: Uuid,
        intent_hash: &str,
    ) -> Result<bool, CiIntegrationError>;

    /// Check if an execution has already been recorded.
    async fn is_duplicate(
        &self,
        pr_number: u64,
        execution_id: Uuid,
    ) -> Result<bool, CiIntegrationError>;

    /// Get all recorded executions for a PR.
    async fn get_executions(&self, pr_number: u64) -> Result<Vec<Uuid>, CiIntegrationError>;

    /// Clean up executions older than the retention period.
    ///
    /// Removes entries that are older than the configured TTL.
    async fn cleanup(&self, retention_hours: u64) -> Result<u64, CiIntegrationError>;
}
