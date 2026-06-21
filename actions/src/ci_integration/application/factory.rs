//! Factory interfaces for constructing CI Integration domain objects.
//!
//! @canonical actions/.pi/architecture/modules/ci-integration.md
//! Implements: Contract Freeze — StatusCheckFactory, PrCommentFactory traits
//! Issue: issue-contract-freeze
//!
//! Factories encapsulate the construction of complex domain objects,
//! allowing implementations to inject dependencies and apply defaults
//! without exposing construction logic to callers.
//!
//! # Contract (Frozen)
//! - Every factory method returns a configured domain object
//! - Validation is applied during construction
//! - No mutable state in factory implementations

use async_trait::async_trait;

use crate::ci_integration::domain::{
    CiIntegrationError, ExecutionSummary, GitHubStatus, StatusCheckState,
};

/// Factory for constructing `GitHubStatus` payloads for commit status checks.
///
/// Implementations handle context naming conventions, target URL generation,
/// and state-to-description mapping logic.
#[async_trait]
pub trait StatusCheckFactory: Send + Sync {
    /// Build a pending `GitHubStatus` for an execution start.
    ///
    /// Sets state to "pending" with the given execution context.
    async fn build_pending_status(
        &self,
        execution_id: &str,
        description: &str,
    ) -> Result<GitHubStatus, CiIntegrationError>;

    /// Build a terminal `GitHubStatus` based on execution outcome.
    ///
    /// Maps the status state to the GitHub API state string and
    /// generates an appropriate description.
    async fn build_outcome_status(
        &self,
        execution_id: &str,
        state: StatusCheckState,
        iterations: u32,
    ) -> Result<GitHubStatus, CiIntegrationError>;

    /// Generate the execution details URL for a status check target_url.
    ///
    /// Links back to the execution run for detailed output.
    async fn build_target_url(&self, execution_id: &str) -> String;

    /// Get the context string for a given status check suffix.
    ///
    /// E.g., `build_context("execution")` → `"rigorix/execution"`.
    fn build_context(&self, suffix: &str) -> String;
}

/// Factory for constructing `ExecutionSummary` payloads for PR comments.
///
/// Handles formatting of execution results into structured summaries
/// suitable for markdown rendering in GitHub comments.
#[async_trait]
pub trait PrCommentFactory: Send + Sync {
    /// Build an `ExecutionSummary` from raw execution data.
    async fn build_summary(
        &self,
        execution_id: &str,
        status: &str,
        quality: Option<&str>,
        steps: Vec<crate::ci_integration::application::dto::ExecutionOutcomeDto>,
    ) -> Result<ExecutionSummary, CiIntegrationError>;

    /// Format an `ExecutionSummary` as GitHub-flavored markdown.
    ///
    /// Includes the bot identifier marker and structured sections
    /// (status, plan, validation, follow-up).
    async fn format_as_markdown(
        &self,
        summary: &ExecutionSummary,
    ) -> Result<String, CiIntegrationError>;
}
