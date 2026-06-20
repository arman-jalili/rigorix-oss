//! Repository interfaces for the Policy Evaluator bounded context.
//!
//! @canonical actions/.pi/architecture/modules/policy-evaluator.md
//! Implements: Contract Freeze — PolicyRepository, OrgPolicyRepository traits
//! Issue: issue-contract-freeze
//!
//! Repositories abstract data access behind interfaces, allowing
//! implementations to use the GitHub API, filesystem, HTTP, or mock
//! storage without coupling domain logic to infrastructure.
//!
//! # Contract (Frozen)
//! - All repository methods are async
//! - All methods return domain error types
//! - No framework-specific annotations on trait definitions
//! - Implementations are hidden behind these interfaces

use async_trait::async_trait;

use crate::policy_evaluator::domain::PolicyError;

/// Repository for reading policy file content from a git repository.
///
/// Abstracts the GitHub API (or local git operations for testing) behind
/// a trait for testability. Implementations can fetch from:
/// - GitHub REST API (`/repos/{owner}/{repo}/contents/{path}`)
/// - Local git checkout (for testing with fixtures)
/// - In-memory cache (for repeated requests)
///
/// # Security
///
/// Implementations MUST fetch from the BASE BRANCH reference, never
/// from the PR branch. This is enforced at the application layer
/// (`PolicyLoadingService`), but repositories should also prefer
/// providing base-branch-only APIs where possible.
///
/// Policy content SHOULD NOT be logged in production to avoid leaking
/// governance rules and patterns.
#[async_trait]
pub trait PolicyRepository: Send + Sync {
    /// Read the raw policy file content from a git reference.
    ///
    /// Fetches the file from the specified repository and reference
    /// (branch or commit SHA).
    ///
    /// # Arguments
    ///
    /// * `policy_path` - Path to the policy file (e.g., ".rigorix/policy.toml").
    /// * `ref_name` - Git reference to fetch from (e.g., "origin/main" or a commit SHA).
    /// * `repo` - Repository identifier in "owner/repo" format.
    ///
    /// # Returns
    ///
    /// The raw file content as a string.
    async fn read_policy(
        &self,
        policy_path: &str,
        ref_name: &str,
        repo: &str,
    ) -> Result<String, PolicyError>;

    /// Check if a policy file exists at the given path and reference.
    ///
    /// Returns `true` if the file exists and is accessible.
    async fn policy_exists(
        &self,
        policy_path: &str,
        ref_name: &str,
        repo: &str,
    ) -> Result<bool, PolicyError>;

    /// Get the repository owner and name.
    ///
    /// Returns `(owner, repo_name)` tuple from environment or configuration.
    async fn get_repo_info(&self) -> Result<(String, String), PolicyError>;

    /// Get the current base branch name.
    ///
    /// Reads from `GITHUB_BASE_REF` environment variable in CI,
    /// or from git configuration locally.
    async fn get_base_ref(&self) -> Result<String, PolicyError>;

    /// Cache policy content for a given key.
    ///
    /// Optional operation — implementations may no-op or use an in-memory cache.
    /// Cache keys should be `{repo}:{ref}:{path}`.
    async fn cache_policy(
        &self,
        repo: &str,
        ref_name: &str,
        path: &str,
        content: &str,
    ) -> Result<(), PolicyError>;

    /// Get cached policy content by key.
    ///
    /// Returns `None` if the policy is not cached.
    async fn get_cached_policy(
        &self,
        repo: &str,
        ref_name: &str,
        path: &str,
    ) -> Result<Option<String>, PolicyError>;

    /// Invalidate cache for a given repository and path.
    ///
    /// Called when a new commit is pushed to the base branch.
    async fn invalidate_cache(&self, repo: &str, policy_path: &str) -> Result<(), PolicyError>;

    /// Get the raw content of a file at a specific commit SHA.
    ///
    /// Used for fetching the exact version of the policy at a given commit.
    async fn read_file_at_commit(
        &self,
        file_path: &str,
        commit_sha: &str,
        repo: &str,
    ) -> Result<String, PolicyError>;
}

/// Repository for reading organization-level policy.
///
/// Organization policies are loaded from a separate source — typically
/// the organization's `.github` repository or a configured URL.
/// These policies enforce minimum governance standards across all
/// repositories in the organization.
///
/// # Sources
///
/// - Organization `.github` repository (`.rigorix/org-policy.toml`)
/// - HTTP(S) URL to a policy file
/// - Filesystem path (for local testing)
#[async_trait]
pub trait OrgPolicyRepository: Send + Sync {
    /// Read the organization-level policy content.
    ///
    /// Fetches the org policy from the configured source.
    /// Returns `None` if the policy is not found at the source.
    ///
    /// # Arguments
    ///
    /// * `source` - The org policy source path or URL.
    /// * `base_ref` - Git reference to fetch from (for git-based sources).
    /// * `repo` - Current repository in "owner/repo" format.
    ///
    /// # Returns
    ///
    /// The raw policy file content, or `None` if not found.
    async fn read_org_policy(
        &self,
        source: &str,
        base_ref: &str,
        repo: &str,
    ) -> Result<Option<String>, PolicyError>;

    /// Check if the organization policy source is accessible.
    ///
    /// Returns `true` if the source can be reached and the policy exists.
    async fn org_policy_source_available(&self, source: &str) -> Result<bool, PolicyError>;

    /// Resolve the org policy source path.
    ///
    /// Given a partial source (e.g., ".rigorix/org-policy.toml"),
    /// resolves it to a full source URL or path based on the
    /// current repository context.
    async fn resolve_source(&self, source: &str, repo: &str) -> Result<String, PolicyError>;

    /// Get the default org policy source for an organization.
    ///
    /// Typically resolves to `.github/rigorix/org-policy.toml` in the
    /// organization's `.github` repository.
    async fn default_org_source(&self, org_name: &str) -> String;
}

/// Repository for caching compiled glob patterns.
///
/// Caches compiled glob patterns to avoid recompilation on repeated
/// evaluations with the same policy. This is an optimization for
/// the case where the same policy is evaluated multiple times within
/// a single action run (e.g., during retry or re-evaluation).
#[async_trait]
pub trait CompiledPatternRepository: Send + Sync {
    /// Store compiled patterns for a policy key.
    async fn store_patterns(
        &self,
        policy_key: &str,
        patterns: &crate::policy_evaluator::domain::CompiledRules,
    ) -> Result<(), PolicyError>;

    /// Get compiled patterns by policy key.
    ///
    /// Returns `None` if not cached.
    async fn get_patterns(
        &self,
        policy_key: &str,
    ) -> Result<Option<crate::policy_evaluator::domain::CompiledRules>, PolicyError>;

    /// Invalidate cached patterns for a given policy key.
    async fn invalidate_patterns(&self, policy_key: &str) -> Result<(), PolicyError>;
}
