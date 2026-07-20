//! ScoredEvaluationService — service trait for evaluation orchestration.
//!
//! @canonical .pi/architecture/modules/scored-evaluation.md#scored-evaluation-service
//! Implements: Contract Freeze — ScoredEvaluationService trait
//! Issue: #673 (scored-evaluation epic)
//!
//! # Contract (Frozen)
//! - Every use case has a corresponding trait method
//! - Input/output types are DTOs defined in `dto/`
//! - All methods are async (use `async-trait` for trait object safety)
//! - No implementation — only contract signatures

use async_trait::async_trait;
use uuid::Uuid;

use crate::scored_evaluation::domain::ScoredEvaluationError;

use super::dto::{EvaluateInput, EvaluateOutput};

/// Application service for orchestrating scored evaluations.
///
/// The `ScoredEvaluationService` is the primary entry point for the
/// scored-evaluation module. It handles:
/// - Evaluating artifacts against rubrics via scoring backends
/// - Retrieving past evaluation results
/// - Listing evaluations for an execution
#[async_trait]
pub trait ScoredEvaluationService: Send + Sync {
    /// Evaluate an artifact against a rubric.
    ///
    /// Orchestrates the full evaluation lifecycle:
    /// 1. Validate input (artifact + rubric)
    /// 2. Resolve the scoring backend by name
    /// 3. Emit `ScoredEvaluationStarted` event
    /// 4. Delegate to `ScoringBackend::evaluate()`
    /// 5. On success: emit `ScoredEvaluationCompleted`, persist result
    /// 6. On failure: emit `ScoredEvaluationFailed`, apply retry/fallback policy
    ///
    /// # Errors
    /// - `ScoredEvaluationError::InvalidArtifact` — artifact is malformed
    /// - `ScoredEvaluationError::InvalidRubric` — rubric is malformed
    /// - `ScoredEvaluationError::BackendNotFound` — no backend configured for name
    /// - `ScoredEvaluationError::BackendError` — backend returned an error
    /// - `ScoredEvaluationError::Timeout` — backend did not respond in time
    async fn evaluate(&self, input: EvaluateInput) -> Result<EvaluateOutput, ScoredEvaluationError>;

    /// Get a specific evaluation result by execution and node ID.
    async fn get_evaluation(
        &self,
        execution_id: Uuid,
        node_id: Uuid,
    ) -> Result<Option<EvaluateOutput>, ScoredEvaluationError>;

    /// List all evaluations for a given execution.
    async fn list_evaluations(
        &self,
        execution_id: Uuid,
    ) -> Result<Vec<EvaluateOutput>, ScoredEvaluationError>;
}
