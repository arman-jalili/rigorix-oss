//! Event payload schemas for the Planning Pipeline bounded context.
//!
//! @canonical .pi/architecture/modules/planning-pipeline.md#events
//! Implements: Contract Freeze — PlanningEvent payload schemas
//! Issue: issue-contract-freeze
//!
//! These events are emitted throughout the 6-phase planning flow — budget pre-check,
//! intent classification, parameter extraction, graph generation, validation, and
//! hash computation. Consumers (orchestrator, audit, TUI) subscribe to these event
//! types via the EventBus.
//!
//! # Contract (Frozen)
//! - Each event carries the full context needed by consumers
//! - No internal implementation details exposed
//! - `execution_id` correlates to the originating execution
//! - Events align with the `ExecutionEvent::PlanningStarted` / `PlanningCompleted`
//!   variants in the event_system module

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::planning::domain::result::PlanningHash;

/// Events emitted by the Planning Pipeline module.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PlanningEvent {
    /// Phase 0: Planning was initiated with a user intent.
    PlanningInitiated {
        /// Globally unique execution identifier.
        execution_id: Uuid,
        /// The original user input text.
        intent: String,
        /// ISO 8601 timestamp.
        timestamp: DateTime<Utc>,
    },

    /// Phase 1: Budget pre-check completed.
    BudgetChecked {
        /// Globally unique execution identifier.
        execution_id: Uuid,
        /// Whether the budget has capacity for planning.
        has_capacity: bool,
        /// Remaining LLM calls after pre-check.
        remaining_calls: u32,
        /// Remaining LLM tokens after pre-check.
        remaining_tokens: u32,
        /// ISO 8601 timestamp.
        timestamp: DateTime<Utc>,
    },

    /// Phase 2: Intent classification completed.
    IntentClassified {
        /// Globally unique execution identifier.
        execution_id: Uuid,
        /// The template selected (or None if no match).
        template_id: Option<String>,
        /// Confidence score of the top match (0.0–1.0).
        confidence: f64,
        /// Whether clarification was requested.
        requires_clarification: bool,
        /// Whether the generator fallback was triggered.
        needs_generator: bool,
        /// Number of alternatives considered.
        alternatives_count: u32,
        /// ISO 8601 timestamp.
        timestamp: DateTime<Utc>,
    },

    /// Phase 3: Parameter extraction completed.
    ParametersExtracted {
        /// Globally unique execution identifier.
        execution_id: Uuid,
        /// The template being parameterised.
        template_id: String,
        /// Number of parameters extracted.
        extracted_count: u32,
        /// Number of required parameters that are missing.
        missing_count: u32,
        /// Whether all required parameters were found.
        complete: bool,
        /// ISO 8601 timestamp.
        timestamp: DateTime<Utc>,
    },

    /// Phase 4: TaskGraph generation completed.
    GraphGenerated {
        /// Globally unique execution identifier.
        execution_id: Uuid,
        /// The template used for generation.
        template_id: String,
        /// Number of nodes in the generated graph.
        node_count: u32,
        /// Whether the graph was generated from a template or generator.
        from_generator: bool,
        /// ISO 8601 timestamp.
        timestamp: DateTime<Utc>,
    },

    /// Phase 5: Plan validation completed.
    PlanValidated {
        /// Globally unique execution identifier.
        execution_id: Uuid,
        /// Whether the plan passed all validation checks.
        passed: bool,
        /// List of validation errors (if any).
        errors: Vec<String>,
        /// List of validation warnings (if any).
        warnings: Vec<String>,
        /// ISO 8601 timestamp.
        timestamp: DateTime<Utc>,
    },

    /// Phase 6: Planning hash computed.
    HashComputed {
        /// Globally unique execution identifier.
        execution_id: Uuid,
        /// The computed deterministic hash.
        planning_hash: PlanningHash,
        /// The template ID used in hash computation.
        template_id: String,
        /// ISO 8601 timestamp.
        timestamp: DateTime<Utc>,
    },

    /// A clarification request was sent to the user.
    ClarificationRequested {
        /// Globally unique execution identifier.
        execution_id: Uuid,
        /// The question asked to the user.
        question: String,
        /// ISO 8601 timestamp.
        timestamp: DateTime<Utc>,
    },

    /// A clarification response was received from the user.
    ClarificationReceived {
        /// Globally unique execution identifier.
        execution_id: Uuid,
        /// The user's response.
        answer: String,
        /// ISO 8601 timestamp.
        timestamp: DateTime<Utc>,
    },

    /// A template generator fallback was triggered.
    GeneratorFallback {
        /// Globally unique execution identifier.
        execution_id: Uuid,
        /// Reason for the fallback (low confidence / no match).
        reason: String,
        /// ISO 8601 timestamp.
        timestamp: DateTime<Utc>,
    },

    /// Planning completed successfully.
    PlanningCompleted {
        /// Globally unique execution identifier.
        execution_id: Uuid,
        /// The template selected for this execution.
        template_id: String,
        /// Model confidence score (0.0–1.0).
        confidence: f64,
        /// The deterministic planning hash.
        planning_hash: PlanningHash,
        /// Number of LLM calls consumed during planning.
        llm_calls_used: u32,
        /// Number of LLM tokens consumed.
        llm_tokens_used: u32,
        /// ISO 8601 timestamp.
        timestamp: DateTime<Utc>,
    },

    /// Planning failed with an error.
    PlanningFailed {
        /// Globally unique execution identifier.
        execution_id: Uuid,
        /// The error that occurred.
        error: String,
        /// The pipeline phase where the error occurred.
        phase: String,
        /// ISO 8601 timestamp.
        timestamp: DateTime<Utc>,
    },

    // -----------------------------------------------------------------------
    // Template Generation Events (fallback path)
    // -----------------------------------------------------------------------
    /// Template generation was triggered as a fallback for low-confidence intent.
    TemplateGenerationStarted {
        /// Globally unique execution identifier.
        execution_id: Uuid,
        /// The user intent that triggered generation.
        intent: String,
        /// Number of retry attempts configured.
        max_retries: u8,
        /// ISO 8601 timestamp.
        timestamp: DateTime<Utc>,
    },

    /// A template was successfully generated by the LLM.
    TemplateGenerated {
        /// Globally unique execution identifier.
        execution_id: Uuid,
        /// The suggested template ID.
        template_id: String,
        /// Number of LLM calls used.
        llm_calls_used: u32,
        /// Number of LLM tokens consumed.
        llm_tokens_used: u32,
        /// Retry attempt that succeeded (0-based).
        attempt: u8,
        /// ISO 8601 timestamp.
        timestamp: DateTime<Utc>,
    },

    /// Template generation failed (all retries exhausted).
    TemplateGenerationFailed {
        /// Globally unique execution identifier.
        execution_id: Uuid,
        /// Error details.
        error: String,
        /// Number of attempts made.
        attempts: u8,
        /// ISO 8601 timestamp.
        timestamp: DateTime<Utc>,
    },

    /// A generation retry was triggered with LLM feedback.
    TemplateGenerationRetrying {
        /// Globally unique execution identifier.
        execution_id: Uuid,
        /// The retry attempt number (0-based).
        attempt: u8,
        /// The error from the previous attempt being fed back.
        feedback: String,
        /// ISO 8601 timestamp.
        timestamp: DateTime<Utc>,
    },

    // -----------------------------------------------------------------------
    // Phase 3: Symbol Validation Events
    // -----------------------------------------------------------------------
    /// Phase 3 symbol validation has started on a generated template.
    SymbolValidationStarted {
        /// Globally unique execution identifier.
        execution_id: Uuid,
        /// The template ID being validated.
        template_id: String,
        /// ISO 8601 timestamp.
        timestamp: DateTime<Utc>,
    },

    /// Phase 3 symbol validation passed — no hallucinated references.
    SymbolValidationPassed {
        /// Globally unique execution identifier.
        execution_id: Uuid,
        /// The template ID that passed validation.
        template_id: String,
        /// Number of symbol references checked.
        references_checked: u32,
        /// Whether any type usage was detected.
        any_type_detected: bool,
        /// ISO 8601 timestamp.
        timestamp: DateTime<Utc>,
    },

    /// Phase 3 symbol validation failed — hallucinated references found.
    SymbolValidationFailed {
        /// Globally unique execution identifier.
        execution_id: Uuid,
        /// The template ID being validated.
        template_id: String,
        /// Number of invalid references found.
        invalid_count: u32,
        /// List of invalid symbol references.
        invalid_references: Vec<String>,
        /// ISO 8601 timestamp.
        timestamp: DateTime<Utc>,
    },
}
