//! ScoredEvaluationServiceImpl — concrete implementation of ScoredEvaluationService.
//!
//! @canonical .pi/architecture/modules/scored-evaluation.md#scored-evaluation-service
//! Implements: ScoredEvaluationService orchestration lifecycle
//! Issue: #684 (scored-evaluation epic)
//!
//! Orchestrates the full evaluation lifecycle:
//! 1. Validate input (artifact + rubric)
//! 2. Resolve the scoring backend by name
//! 3. Emit ScoredEvaluationStarted event
//! 4. Delegate to ScoringBackend::evaluate()
//! 5. On success: emit ScoredEvaluationCompleted, persist result
//! 6. On failure: emit ScoredEvaluationFailed, apply retry/fallback policy

use async_trait::async_trait;
use chrono::Utc;
use std::collections::HashMap;
use uuid::Uuid;

use crate::scored_evaluation::domain::{
    ScoredEvaluationError, ScoredEvaluationEvent, ScoringBackend, ScoringResult,
};
use crate::scored_evaluation::infrastructure::EvaluationRepository;

use super::dto::{EvaluateInput, EvaluateOutput};
use super::service::ScoredEvaluationService;

/// Maximum number of retry attempts for transient failures.
const MAX_RETRIES: u32 = 3;

/// Backoff base delay in milliseconds.
const BACKOFF_BASE_MS: u64 = 200;

/// Concrete implementation of the ScoredEvaluationService.
///
/// Orchestrates the evaluation lifecycle: validates input, resolves backend,
/// emits events, delegates to backend, persists results, and handles retries.
pub struct ScoredEvaluationServiceImpl {
    /// Registry of scoring backends by name.
    backends: HashMap<String, Box<dyn ScoringBackend>>,

    /// Repository for persisting evaluation results.
    repository: Box<dyn EvaluationRepository>,

    /// Event callback for publishing domain events.
    event_sink: Option<Box<dyn EventSink>>,
}

/// Trait for publishing scored evaluation events.
///
/// This allows the service to emit events without depending directly
/// on the EventBus, keeping the service testable.
#[async_trait]
pub trait EventSink: Send + Sync {
    /// Publish a scored evaluation event.
    async fn publish(&self, event: ScoredEvaluationEvent);
}

impl ScoredEvaluationServiceImpl {
    /// Create a new service with the given backends and repository.
    pub fn new(
        backends: HashMap<String, Box<dyn ScoringBackend>>,
        repository: Box<dyn EvaluationRepository>,
    ) -> Self {
        Self {
            backends,
            repository,
            event_sink: None,
        }
    }

    /// Set the event sink for publishing events.
    pub fn with_event_sink(mut self, sink: Box<dyn EventSink>) -> Self {
        self.event_sink = Some(sink);
        self
    }

    /// Publish an event via the event sink, if configured.
    async fn publish_event(&self, event: ScoredEvaluationEvent) {
        if let Some(sink) = &self.event_sink {
            sink.publish(event).await;
        }
    }

    /// Validate that the artifact and rubric are well-formed.
    fn validate_input(&self, input: &EvaluateInput) -> Result<(), ScoredEvaluationError> {
        if input.artifact.is_null() || input.artifact.is_array() && input.artifact.as_array().map_or(true, |a| a.is_empty()) {
            return Err(ScoredEvaluationError::InvalidArtifact(
                "Artifact must be a non-empty JSON value".to_string(),
            ));
        }
        if !input.rubric.is_inline() && !input.rubric.is_reference() {
            return Err(ScoredEvaluationError::InvalidRubric(
                "Rubric must have a valid source type (inline or reference)".to_string(),
            ));
        }
        Ok(())
    }

    /// Resolve a scoring backend by name.
    fn resolve_backend(&self, name: &str) -> Result<&dyn ScoringBackend, ScoredEvaluationError> {
        self.backends
            .get(name)
            .map(|b| b.as_ref())
            .ok_or_else(|| ScoredEvaluationError::BackendNotFound(name.to_string()))
    }

    /// Execute the evaluation with retry logic for transient failures.
    async fn execute_with_retry(
        &self,
        backend: &dyn ScoringBackend,
        artifact: &serde_json::Value,
        rubric: &crate::scored_evaluation::domain::Rubric,
    ) -> Result<ScoringResult, ScoredEvaluationError> {
        let mut last_error = ScoredEvaluationError::Internal("no attempt made".to_string());

        for attempt in 0..=MAX_RETRIES {
            match backend.evaluate(artifact, rubric).await {
                Ok(result) => return Ok(result),
                Err(e) if e.is_retriable() && attempt < MAX_RETRIES => {
                    last_error = e;
                    let delay_ms = BACKOFF_BASE_MS * 2u64.pow(attempt);
                    tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
                }
                Err(e) => return Err(e),
            }
        }

        Err(last_error)
    }
}

#[async_trait]
impl ScoredEvaluationService for ScoredEvaluationServiceImpl {
    async fn evaluate(&self, input: EvaluateInput) -> Result<EvaluateOutput, ScoredEvaluationError> {
        // 1. Validate input
        self.validate_input(&input)?;

        // 2. Resolve backend
        // Find the first configured backend
        let (backend_name, backend) = self.backends.iter().next()
            .ok_or_else(|| ScoredEvaluationError::BackendNotFound("no backends configured".to_string()))?;

        // 3. Emit started event
        let node_id = input.context.node_id.to_string();
        self.publish_event(ScoredEvaluationEvent::ScoredEvaluationStarted {
            node_id: node_id.clone(),
            execution_id: input.context.execution_id,
            backend: backend_name.clone(),
            timestamp: Utc::now(),
        }).await;

        // 4. Execute evaluation with retry
        let result = self.execute_with_retry(
            backend.as_ref(),
            &input.artifact,
            &input.rubric,
        ).await;

        let timestamp = Utc::now();

        match result {
            Ok(scoring_result) => {
                // 5a. Emit completed event
                self.publish_event(ScoredEvaluationEvent::ScoredEvaluationCompleted {
                    node_id: node_id.clone(),
                    execution_id: input.context.execution_id,
                    result: scoring_result.clone(),
                    timestamp,
                }).await;

                // Persist result
                let output = EvaluateOutput::new(
                    scoring_result,
                    input.context.execution_id,
                    input.context.node_id,
                    input.context.node_name,
                    timestamp,
                );
                self.repository.save(&output).await?;

                Ok(output)
            }
            Err(error) => {
                // 5b. Emit failed event
                self.publish_event(ScoredEvaluationEvent::ScoredEvaluationFailed {
                    node_id: node_id.clone(),
                    execution_id: input.context.execution_id,
                    error: error.to_string(),
                    timestamp,
                }).await;

                Err(error)
            }
        }
    }

    async fn get_evaluation(
        &self,
        execution_id: Uuid,
        node_id: Uuid,
    ) -> Result<Option<EvaluateOutput>, ScoredEvaluationError> {
        self.repository.get(execution_id, node_id).await
    }

    async fn list_evaluations(
        &self,
        execution_id: Uuid,
    ) -> Result<Vec<EvaluateOutput>, ScoredEvaluationError> {
        self.repository.list(execution_id).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scored_evaluation::domain::{Rubric, ScoringResult, ScoreDimension};
    use std::collections::HashMap;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    struct MockBackend {
        result: Option<ScoringResult>,
        error: Option<ScoredEvaluationError>,
        health: bool,
    }

    #[async_trait]
    impl ScoringBackend for MockBackend {
        async fn evaluate(
            &self,
            _artifact: &serde_json::Value,
            _rubric: &Rubric,
        ) -> Result<ScoringResult, ScoredEvaluationError> {
            if let Some(err) = &self.error {
                return Err(err.clone());
            }
            Ok(self.result.clone().unwrap())
        }

        fn backend_name(&self) -> &'static str {
            "mock"
        }

        async fn health_check(&self) -> Result<bool, ScoredEvaluationError> {
            Ok(self.health)
        }
    }

    struct MockRepository {
        results: Arc<Mutex<Vec<EvaluateOutput>>>,
    }

    #[async_trait]
    impl EvaluationRepository for MockRepository {
        async fn save(&self, output: &EvaluateOutput) -> Result<(), ScoredEvaluationError> {
            let mut results = self.results.lock().await;
            results.push(output.clone());
            Ok(())
        }

        async fn get(
            &self,
            _execution_id: Uuid,
            _node_id: Uuid,
        ) -> Result<Option<EvaluateOutput>, ScoredEvaluationError> {
            let results = self.results.lock().await;
            Ok(results.first().cloned())
        }

        async fn list(
            &self,
            _execution_id: Uuid,
        ) -> Result<Vec<EvaluateOutput>, ScoredEvaluationError> {
            let results = self.results.lock().await;
            Ok(results.clone())
        }

        async fn delete_by_execution(
            &self,
            _execution_id: Uuid,
        ) -> Result<(), ScoredEvaluationError> {
            let mut results = self.results.lock().await;
            results.clear();
            Ok(())
        }
    }

    struct MockEventSink {
        events: Arc<Mutex<Vec<ScoredEvaluationEvent>>>,
    }

    #[async_trait]
    impl EventSink for MockEventSink {
        async fn publish(&self, event: ScoredEvaluationEvent) {
            let mut events = self.events.lock().await;
            events.push(event);
        }
    }

    fn make_result() -> ScoringResult {
        let mut dims = HashMap::new();
        dims.insert(
            "correctness".to_string(),
            ScoreDimension::new(0.95, 1.0, "Correctness", true),
        );
        ScoringResult::new(true, dims, "All good", "mock", 100, None)
    }

    fn make_service() -> ScoredEvaluationServiceImpl {
        let mut backends: HashMap<String, Box<dyn ScoringBackend>> = HashMap::new();
        backends.insert(
            "mock".to_string(),
            Box::new(MockBackend {
                result: Some(make_result()),
                error: None,
                health: true,
            }),
        );

        let results = Arc::new(Mutex::new(Vec::new()));
        let repository = Box::new(MockRepository { results: results.clone() });

        ScoredEvaluationServiceImpl::new(backends, repository)
    }

    #[tokio::test]
    async fn test_evaluate_success() {
        let service = make_service();
        let input = EvaluateInput::new(
            serde_json::json!({"code": "fn main() {}"}),
            Rubric::inline(serde_json::json!({"quality": 0.9})),
            Uuid::new_v4(),
            Uuid::new_v4(),
            "test-node",
        );
        let result = service.evaluate(input).await;
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.result.passed);
    }

    #[tokio::test]
    async fn test_evaluate_invalid_artifact() {
        let service = make_service();
        let input = EvaluateInput::new(
            serde_json::Value::Null,
            Rubric::inline(serde_json::json!({"quality": 0.9})),
            Uuid::new_v4(),
            Uuid::new_v4(),
            "test-node",
        );
        let result = service.evaluate(input).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ScoredEvaluationError::InvalidArtifact(_)));
    }

    #[tokio::test]
    async fn test_evaluate_backend_error() {
        let mut backends: HashMap<String, Box<dyn ScoringBackend>> = HashMap::new();
        backends.insert(
            "failing".to_string(),
            Box::new(MockBackend {
                result: None,
                error: Some(ScoredEvaluationError::BackendError("failure".to_string())),
                health: false,
            }),
        );

        let results = Arc::new(Mutex::new(Vec::new()));
        let repository = Box::new(MockRepository { results });

        let service = ScoredEvaluationServiceImpl::new(backends, repository);
        let input = EvaluateInput::new(
            serde_json::json!({"code": "test"}),
            Rubric::inline(serde_json::json!({"quality": 0.9})),
            Uuid::new_v4(),
            Uuid::new_v4(),
            "test-node",
        );
        let result = service.evaluate(input).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_get_evaluation() {
        let service = make_service();
        let exec_id = Uuid::new_v4();
        let node_id = Uuid::new_v4();
        let input = EvaluateInput::new(
            serde_json::json!({"code": "test"}),
            Rubric::inline(serde_json::json!({"quality": 0.9})),
            exec_id,
            node_id,
            "test-node",
        );
        service.evaluate(input).await.unwrap();
        let result = service.get_evaluation(exec_id, node_id).await.unwrap();
        assert!(result.is_some());
    }
}
