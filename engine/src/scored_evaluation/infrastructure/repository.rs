//! EvaluationRepository — repository interface for persisting evaluation results.
//!
//! @canonical .pi/architecture/modules/scored-evaluation.md#evaluation-repository
//! Implements: Contract Freeze — EvaluationRepository trait
//! Issue: #673 (scored-evaluation epic)
//!
//! # Contract (Frozen)
//! - Abstracts data access behind an interface
//! - All methods are async
//! - Methods return domain error types
//! - No framework-specific annotations on trait definitions

use async_trait::async_trait;
use uuid::Uuid;

use crate::scored_evaluation::domain::ScoredEvaluationError;

use crate::scored_evaluation::application::dto::EvaluateOutput;

/// Repository for persisting and retrieving evaluation results.
///
/// Implementations can store results in filesystem (JSON files),
/// SQLite, or in-memory for testing.
#[async_trait]
pub trait EvaluationRepository: Send + Sync {
    /// Save an evaluation result.
    async fn save(&self, output: &EvaluateOutput) -> Result<(), ScoredEvaluationError>;

    /// Get an evaluation result by execution and node ID.
    async fn get(
        &self,
        execution_id: Uuid,
        node_id: Uuid,
    ) -> Result<Option<EvaluateOutput>, ScoredEvaluationError>;

    /// List all evaluations for a given execution.
    async fn list(
        &self,
        execution_id: Uuid,
    ) -> Result<Vec<EvaluateOutput>, ScoredEvaluationError>;

    /// Delete all evaluations for a given execution.
    async fn delete_by_execution(
        &self,
        execution_id: Uuid,
    ) -> Result<(), ScoredEvaluationError>;
}
