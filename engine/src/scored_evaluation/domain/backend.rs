//! ScoringBackend — pluggable scoring backend trait (domain interface).
//!
//! @canonical .pi/architecture/modules/scored-evaluation.md#scoring-backend
//! Implements: Contract Freeze — ScoringBackend trait
//! Issue: #673 (scored-evaluation epic)
//!
//! # Contract (Frozen)
//! - `ScoringBackend` is the domain-level contract for any evaluation backend
//! - Lives in the domain layer — implementations live in infrastructure
//! - All implementations must satisfy this trait (MCP, HTTP, Local)
//! - `evaluate()` is async and returns `ScoringResult` or `ScoredEvaluationError`
//! - `health_check()` allows pre-flight validation before evaluation
//! - Trait is object-safe (uses `async_trait`)

use async_trait::async_trait;

use super::error::ScoredEvaluationError;
use super::result::ScoringResult;
use super::rubric::Rubric;

/// Pluggable scoring backend interface — domain-level contract.
///
/// The `ScoringBackend` trait defines the contract that all evaluation
/// backends must satisfy. Rigorix defines this protocol; external systems
/// (RuntimeAI, custom HTTP services, local scripts) implement it.
///
/// # Implementations
///
/// | Backend | Transport | Protocol |
/// |---------|-----------|----------|
/// | MCPBackend | MCP `rigorix_evaluate_artifact` request | Rigorix Scoring Protocol over MCP |
/// | HTTPBackend | HTTP POST | Rigorix Scoring Protocol (JSON) over REST |
/// | LocalBackend | Subprocess execution | Rigorix Scoring Protocol (stdin/stdout) |
#[async_trait]
pub trait ScoringBackend: Send + Sync {
    /// Evaluate an artifact against a rubric.
    ///
    /// Sends the artifact and rubric to the backend, which returns a
    /// multidimensional scoring result.
    ///
    /// # Errors
    /// - `ScoredEvaluationError::BackendError` — backend returned an error
    /// - `ScoredEvaluationError::Timeout` — backend did not respond in time
    /// - `ScoredEvaluationError::BackendUnavailable` — backend is down
    /// - `ScoredEvaluationError::InvalidRubric` — rubric format rejected
    async fn evaluate(
        &self,
        artifact: &serde_json::Value,
        rubric: &Rubric,
    ) -> Result<ScoringResult, ScoredEvaluationError>;

    /// Returns the name of this backend (e.g., "mcp", "http", "local").
    fn backend_name(&self) -> &'static str;

    /// Check whether the backend is healthy and reachable.
    ///
    /// Returns `true` if the backend is operational, `false` otherwise.
    /// On error, returns a `ScoredEvaluationError` with details.
    async fn health_check(&self) -> Result<bool, ScoredEvaluationError>;
}
