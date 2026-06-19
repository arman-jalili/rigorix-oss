//! CoreOrchestratorError — Root error type for the Rigorix orchestrator.
//!
//! @canonical .pi/architecture/modules/error-handling.md#coreorchestratorerror
//! Implements: Contract Freeze — CoreOrchestratorError enum
//! Issue: #186
//!
//! This is the root error type that aggregates all domain-specific errors
//! via `#[from]` for consistent error propagation and Display chains.
//!
//! # Design
//!
//! Each domain module defines its own error enum (e.g., `DagError`,
//! `PlanningError`). The `CoreOrchestratorError` wraps them all via `#[from]`,
//! allowing seamless propagation with the `?` operator.
//!
//! # Contract (Frozen)
//! - Every sub-error type has a corresponding variant with `#[from]`
//! - Standard library errors (io::Error, serde_json::Error) are included
//! - `Cancelled` variant for cancellation signals
//! - `Http` variant for HTTP error responses with structured context
//! - Implements `std::error::Error` for library compatibility
//! - No `anyhow` in library code — use thiserror everywhere

use thiserror::Error;

use crate::audit::domain::AuditError;
use crate::budget_tracking::domain::LlmBudgetError;
use crate::cancellation::domain::CancellationError;
use crate::configuration::domain::ConfigurationError;
use crate::dag_engine::domain::DagError;
use crate::enforcement::domain::EnforcementError;
use crate::event_system::domain::EventSystemError;
use crate::execution_engine::domain::ExecutionError;
use crate::failure_classification::domain::FailureClassificationError;
use crate::orchestrator::domain::OrchestratorError;
use crate::planning::domain::PlanningError;
use crate::quality_gates::domain::QualityGateError;
use crate::repo_engine::domain::RepoEngineError;
use crate::state_persistence::domain::StateError;
use crate::recovery_recipes::domain::RecoveryError;
use crate::templates::domain::TemplateError;
use crate::tools::domain::ToolError;

/// Root error type for the Rigorix orchestrator.
///
/// Aggregates all domain-specific errors via `#[from]` for seamless
/// error propagation. Every public-facing function should return
/// `Result<_, CoreOrchestratorError>` or a domain-specific error
/// that converts to it.
#[derive(Debug, Error)]
pub enum CoreOrchestratorError {
    // ------------------------------------------------------------------ /
    // Module-level sub-errors (via #[from])
    // ------------------------------------------------------------------ /
    /// DAG engine error — graph construction, validation, lifecycle.
    #[error("DAG error: {0}")]
    Dag(#[from] DagError),

    /// Planning pipeline error — template matching, classification,
    /// parameter extraction, validation.
    #[error("Planning error: {0}")]
    Planning(#[from] PlanningError),

    /// Enforcement error — policy violations, budget limits,
    /// execution limits.
    #[error("Enforcement error: {0}")]
    Enforcement(#[from] EnforcementError),

    /// Budget tracking error — LLM call/token budget exceeded.
    #[error("Budget error: {0}")]
    Budget(#[from] LlmBudgetError),

    /// Execution error — task failures, timeouts, fallback handling.
    #[error("Execution error: {0}")]
    Execution(#[from] ExecutionError),

    /// Tool error — tool execution failures, path denials,
    /// validation errors.
    #[error("Tool error: {0}")]
    Tool(#[from] ToolError),

    /// Repo engine (symbol graph) error — indexing, lookup, parsing.
    #[error("Symbol graph error: {0}")]
    SymbolGraph(#[from] RepoEngineError),

    /// Configuration error — file not found, parse errors,
    /// invalid configuration.
    #[error("Configuration error: {0}")]
    Configuration(#[from] ConfigurationError),

    /// Cancellation error — task not found, already cancelled,
    /// shutdown timeout.
    #[error("Cancellation error: {0}")]
    Cancellation(#[from] CancellationError),

    /// Event system error — publish, subscribe, drain failures.
    #[error("Event system error: {0}")]
    EventSystem(#[from] EventSystemError),

    /// Audit error — send failures, serialisation, queue full.
    #[error("Audit error: {0}")]
    Audit(#[from] AuditError),

    /// State persistence error — save/load failures, corruption,
    /// lock errors.
    #[error("State error: {0}")]
    State(#[from] StateError),

    /// Template error — parse, validation, generation failures.
    #[error("Template error: {0}")]
    Template(#[from] TemplateError),

    /// Orchestrator error — lifecycle failures, sub-service wiring.
    #[error("Orchestrator error: {0}")]
    Orchestrator(#[from] OrchestratorError),

    /// Recovery recipes error — no recipe, max attempts, step failures.
    #[error("Recovery error: {0}")]
    Recovery(#[from] RecoveryError),

    /// Quality gates error — scope classification, contract evaluation.
    #[error("Quality gate error: {0}")]
    QualityGate(#[from] QualityGateError),

    /// Failure classification error — classification failures,
    /// missing strategies.
    #[error("Failure classification error: {0}")]
    FailureClassification(#[from] FailureClassificationError),

    // ------------------------------------------------------------------ /
    // Standard library wrappers (via #[from])
    // ------------------------------------------------------------------ /
    /// I/O error wrapper.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON serialisation/deserialisation error wrapper.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    // ------------------------------------------------------------------ /
    // Signals and structured errors (manual conversion)
    // ------------------------------------------------------------------ /
    /// Operation was cancelled.
    ///
    /// Carries a human-readable description of why the operation was
    /// cancelled. This is typically generated by the Cancellation module
    /// but kept as a separate variant for clarity.
    #[error("Operation cancelled: {0}")]
    Cancelled(String),

    /// HTTP error with structured diagnostics.
    ///
    /// Used when making outbound HTTP requests to external services.
    /// Carries the HTTP status code, response body preview, and the
    /// URL that was being called.
    #[error("HTTP error: {message} (status: {status}, url: {url})")]
    Http {
        /// Human-readable error message.
        message: String,
        /// HTTP status code (e.g., 404, 500).
        status: u16,
        /// The URL that returned the error.
        url: String,
    },
}

impl CoreOrchestratorError {
    /// Check if the error represents a transient failure that can be retried.
    ///
    /// Transient errors include I/O errors, HTTP 5xx errors, and
    /// certain domain-specific errors that may succeed on retry.
    pub fn is_retriable(&self) -> bool {
        match self {
            // I/O errors are often transient (e.g., file lock contention)
            CoreOrchestratorError::Io(_) => true,
            // HTTP 5xx errors are typically transient; 4xx errors are not
            CoreOrchestratorError::Http { status, .. } if *status >= 500 => true,
            CoreOrchestratorError::Http { .. } => false,
            // Delegate to domain-specific retriable logic
            CoreOrchestratorError::Dag(e) => e.is_retriable(),
            CoreOrchestratorError::Planning(e) => e.is_retriable(),
            CoreOrchestratorError::Enforcement(e) => e.is_retriable(),
            CoreOrchestratorError::Budget(e) => e.is_retriable(),
            CoreOrchestratorError::Execution(e) => e.is_retriable(),
            CoreOrchestratorError::Tool(e) => e.is_retriable(),
            CoreOrchestratorError::SymbolGraph(e) => e.is_retriable(),
            CoreOrchestratorError::Configuration(e) => e.is_retriable(),
            CoreOrchestratorError::Cancellation(e) => e.is_retriable(),
            CoreOrchestratorError::EventSystem(e) => e.is_retriable(),
            CoreOrchestratorError::Audit(e) => e.is_retriable(),
            CoreOrchestratorError::State(e) => e.is_retriable(),
            CoreOrchestratorError::Template(e) => e.is_retriable(),
            CoreOrchestratorError::Orchestrator(e) => e.is_retriable(),
            CoreOrchestratorError::Recovery(e) => e.is_retriable(),
            CoreOrchestratorError::QualityGate(e) => e.is_retriable(),
            CoreOrchestratorError::FailureClassification(e) => e.is_retriable(),
            // JSON deserialization errors are not retriable — the input is malformed
            CoreOrchestratorError::Json(_) => false,
            // Cancellation is intentional, not transient
            CoreOrchestratorError::Cancelled(_) => false,
        }
    }

    /// Get a machine-readable error code for this error.
    pub fn error_code(&self) -> &'static str {
        match self {
            CoreOrchestratorError::Dag(_) => "DAG_ERROR",
            CoreOrchestratorError::Planning(_) => "PLANNING_ERROR",
            CoreOrchestratorError::Enforcement(_) => "ENFORCEMENT_ERROR",
            CoreOrchestratorError::Budget(_) => "BUDGET_ERROR",
            CoreOrchestratorError::Execution(_) => "EXECUTION_ERROR",
            CoreOrchestratorError::Tool(_) => "TOOL_ERROR",
            CoreOrchestratorError::SymbolGraph(_) => "SYMBOL_GRAPH_ERROR",
            CoreOrchestratorError::Configuration(_) => "CONFIGURATION_ERROR",
            CoreOrchestratorError::Cancellation(_) => "CANCELLATION_ERROR",
            CoreOrchestratorError::EventSystem(_) => "EVENT_SYSTEM_ERROR",
            CoreOrchestratorError::Audit(_) => "AUDIT_ERROR",
            CoreOrchestratorError::State(_) => "STATE_ERROR",
            CoreOrchestratorError::Template(_) => "TEMPLATE_ERROR",
            CoreOrchestratorError::Orchestrator(_) => "ORCHESTRATOR_ERROR",
            CoreOrchestratorError::Recovery(_) => "RECOVERY_ERROR",
            CoreOrchestratorError::QualityGate(_) => "QUALITY_GATE_ERROR",
            CoreOrchestratorError::FailureClassification(_) => "FAILURE_CLASSIFICATION_ERROR",
            CoreOrchestratorError::Io(_) => "IO_ERROR",
            CoreOrchestratorError::Json(_) => "JSON_ERROR",
            CoreOrchestratorError::Cancelled(_) => "CANCELLED",
            CoreOrchestratorError::Http { .. } => "HTTP_ERROR",
        }
    }

    /// Get the HTTP status code that best represents this error.
    pub fn http_status(&self) -> u16 {
        match self {
            CoreOrchestratorError::Dag(_) => 500,
            CoreOrchestratorError::Planning(_) => 400,
            CoreOrchestratorError::Enforcement(_) => 429,
            CoreOrchestratorError::Budget(_) => 429,
            CoreOrchestratorError::Execution(_) => 500,
            CoreOrchestratorError::Tool(_) => 500,
            CoreOrchestratorError::SymbolGraph(_) => 500,
            CoreOrchestratorError::Configuration(_) => 500,
            CoreOrchestratorError::Cancellation(_) => 400,
            CoreOrchestratorError::EventSystem(_) => 500,
            CoreOrchestratorError::Audit(_) => 500,
            CoreOrchestratorError::State(_) => 500,
            CoreOrchestratorError::Template(_) => 400,
            CoreOrchestratorError::Orchestrator(_) => 500,
            CoreOrchestratorError::Recovery(_) => 500,
            CoreOrchestratorError::QualityGate(_) => 500,
            CoreOrchestratorError::FailureClassification(_) => 500,
            CoreOrchestratorError::Io(_) => 500,
            CoreOrchestratorError::Json(_) => 400,
            CoreOrchestratorError::Cancelled(_) => 499,
            CoreOrchestratorError::Http { status, .. } => *status,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_code_mapping() {
        assert_eq!(
            CoreOrchestratorError::Dag(DagError::CycleDetected { found: 0, total: 0 }).error_code(),
            "DAG_ERROR"
        );
        assert_eq!(
            CoreOrchestratorError::Io(std::io::Error::new(std::io::ErrorKind::NotFound, "test"))
                .error_code(),
            "IO_ERROR"
        );
        assert_eq!(
            CoreOrchestratorError::Cancelled("test".to_string()).error_code(),
            "CANCELLED"
        );
    }

    #[test]
    fn test_http_status_mapping() {
        assert_eq!(
            CoreOrchestratorError::Enforcement(EnforcementError::ExecutionLimitReached {
                limit_type: "max_tool_calls".to_string(),
                current: 10,
                max: 10,
            })
            .http_status(),
            429
        );
        assert_eq!(
            CoreOrchestratorError::Http {
                message: "Not Found".to_string(),
                status: 404,
                url: "https://example.com".to_string(),
            }
            .http_status(),
            404
        );
        assert_eq!(
            CoreOrchestratorError::Cancelled("by user".to_string()).http_status(),
            499
        );
    }

    #[test]
    fn test_is_retriable_io_error() {
        let err = CoreOrchestratorError::Io(std::io::Error::new(
            std::io::ErrorKind::WouldBlock,
            "would block",
        ));
        assert!(err.is_retriable());
    }

    #[test]
    fn test_is_retriable_http_5xx() {
        let err = CoreOrchestratorError::Http {
            message: "Internal Server Error".to_string(),
            status: 500,
            url: "https://example.com".to_string(),
        };
        assert!(err.is_retriable());
    }

    #[test]
    fn test_is_retriable_http_4xx() {
        let err = CoreOrchestratorError::Http {
            message: "Bad Request".to_string(),
            status: 400,
            url: "https://example.com".to_string(),
        };
        assert!(!err.is_retriable());
    }

    #[test]
    fn test_all_variants_have_error_code() {
        // This test verifies every variant has a non-empty error_code
        let variants: Vec<CoreOrchestratorError> = vec![
            CoreOrchestratorError::Dag(DagError::CycleDetected { found: 0, total: 0 }),
            CoreOrchestratorError::Planning(PlanningError::Cancelled),
            CoreOrchestratorError::Enforcement(EnforcementError::ExecutionLimitReached {
                limit_type: "test".to_string(),
                current: 0,
                max: 10,
            }),
            CoreOrchestratorError::Budget(LlmBudgetError::MaxCallsExceeded { used: 0, max: 10 }),
            CoreOrchestratorError::Execution(ExecutionError::InvalidState {
                reason: "test".to_string(),
            }),
            CoreOrchestratorError::Tool(ToolError::NotFound("test".to_string())),
            CoreOrchestratorError::SymbolGraph(RepoEngineError::SymbolNotFound {
                name: "test".to_string(),
                suggestions: vec![],
            }),
            CoreOrchestratorError::Configuration(ConfigurationError::NotFound {
                path: "test".to_string(),
                config_source: crate::configuration::domain::ConfigSource::Default,
            }),
            CoreOrchestratorError::Cancellation(CancellationError::NoSubscribers),
            CoreOrchestratorError::EventSystem(EventSystemError::NoSubscribers { sequence: 0 }),
            CoreOrchestratorError::Audit(AuditError::NotConfigured {
                missing_field: "test".to_string(),
            }),
            CoreOrchestratorError::State(StateError::StateNotFound {
                execution_id: "test".to_string(),
            }),
            CoreOrchestratorError::Template(TemplateError::NotFound {
                id: "test".to_string(),
                available: vec![],
            }),
            CoreOrchestratorError::Orchestrator(OrchestratorError::PlanningFailed {
                detail: "test".to_string(),
                intent: "test".to_string(),
            }),
            CoreOrchestratorError::Recovery(RecoveryError::NoRecipe(
                crate::recovery_recipes::domain::FailureScenario::CompileError,
            )),
            CoreOrchestratorError::QualityGate(QualityGateError::ScopeClassificationFailed {
                reason: "test".to_string(),
            }),
            CoreOrchestratorError::FailureClassification(
                FailureClassificationError::ClassificationFailed {
                    message: "test".to_string(),
                    reason: "test".to_string(),
                },
            ),
            CoreOrchestratorError::Io(std::io::Error::new(std::io::ErrorKind::NotFound, "test")),
            CoreOrchestratorError::Json(
                serde_json::from_str::<serde_json::Value>("invalid").unwrap_err(),
            ),
            CoreOrchestratorError::Cancelled("test".to_string()),
            CoreOrchestratorError::Http {
                message: "test".to_string(),
                status: 500,
                url: "https://example.com".to_string(),
            },
        ];

        for variant in &variants {
            assert!(
                !variant.error_code().is_empty(),
                "error_code() returned empty for {:?}",
                variant
            );
            assert!(
                std::matches!(variant.http_status(), 400 | 429 | 499 | 500 | 404),
                "Unexpected status for {:?}: {}",
                variant,
                variant.http_status()
            );
        }
    }
}
