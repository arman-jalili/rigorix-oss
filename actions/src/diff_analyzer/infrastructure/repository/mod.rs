//! Repository interfaces for the Diff Analyzer bounded context.
//!
//! @canonical actions/.pi/architecture/modules/diff-analyzer.md
//! Implements: Contract Freeze — DiffRepository trait
//! Issue: issue-contract-freeze
//!
//! Repositories abstract data access behind interfaces, allowing
//! implementations to use the GitHub API, local filesystem, or mock
//! storage without coupling domain logic to infrastructure.
//!
//! # Contract (Frozen)
//! - All repository methods are async
//! - All methods return domain error types
//! - No framework-specific annotations on trait definitions
//! - Implementations are hidden behind these interfaces

use async_trait::async_trait;

use crate::diff_analyzer::domain::DiffAnalyzerError;

/// Repository for fetching and caching PR diffs.
///
/// Abstracts the GitHub API (or local file system for testing) behind
/// a trait for testability. Implementations can fetch from:
/// - GitHub REST API (`/repos/{owner}/{repo}/pulls/{number}`)
/// - Local file system (test fixtures)
/// - In-memory cache
///
/// # Security
/// - Diff content MUST NOT be logged in production (may contain secrets)
/// - Rate limiting is handled by the implementation
#[async_trait]
pub trait DiffRepository: Send + Sync {
    /// Fetch the raw diff for a pull request.
    ///
    /// Fetches the unified diff from the GitHub API using the
    /// `application/vnd.github.v3.diff` media type.
    ///
    /// # Arguments
    /// * `pr_number` - The PR number to fetch the diff for.
    ///
    /// # Returns
    /// * The raw unified diff as a string.
    async fn fetch_diff(&self, pr_number: u64) -> Result<String, DiffAnalyzerError>;

    /// Fetch the raw diff for a pull request with a specific commit SHA.
    ///
    /// Useful for analyzing diffs at a specific point in the PR history.
    async fn fetch_diff_at_sha(
        &self,
        pr_number: u64,
        sha: &str,
    ) -> Result<String, DiffAnalyzerError>;

    /// Check if a PR exists and is accessible.
    ///
    /// Returns `true` if the PR exists and the implementation has access.
    async fn check_pr_exists(&self, pr_number: u64) -> Result<bool, DiffAnalyzerError>;

    /// Get the repository owner and name.
    ///
    /// Returns `(owner, repo_name)` tuple.
    async fn get_repo_info(&self) -> Result<(String, String), DiffAnalyzerError>;

    /// Cache a parsed diff for later retrieval.
    ///
    /// Optional operation — implementations may no-op or use an in-memory cache.
    /// Cache keys should be `{pr_number}:{sha}`.
    async fn cache_diff(
        &self,
        pr_number: u64,
        sha: &str,
        diff: &str,
    ) -> Result<(), DiffAnalyzerError>;

    /// Get a cached diff by PR number and SHA.
    ///
    /// Returns `None` if the diff is not cached.
    async fn get_cached_diff(
        &self,
        pr_number: u64,
        sha: &str,
    ) -> Result<Option<String>, DiffAnalyzerError>;

    /// Invalidate cache for a given PR.
    ///
    /// Called when a PR is synchronized (new commits pushed).
    async fn invalidate_cache(&self, pr_number: u64) -> Result<(), DiffAnalyzerError>;
}
