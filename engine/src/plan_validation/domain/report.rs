//! ValidationReport — structured report produced after validation completes.
//!
//! @canonical .pi/architecture/modules/plan-validation.md#report
//! Implements: Contract Freeze — ValidationReport, ValidationIterationReport
//! Issue: issue-contract-freeze
//!
//! Produced after validation completes (success or failure). Contains
//! the full execution history: iteration count, duration, cumulative
//! token usage, per-iteration failure details, and the final validated
//! template (if successful).
//!
//! # Contract (Frozen)
//! - Report is produced regardless of outcome (success or failure)
//! - Per-iteration reports include failures, tokens, duration, and fixes applied
//! - Serialization support for audit trails and API responses
//! - No implementation logic beyond constructors and field accessors

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::failure_parser::domain::TemplateFailure;
use crate::templates::domain::Template;

use super::outcome::ValidationOutcome;

/// Structured validation report produced after the loop completes.
///
/// Contains the full execution history across all iterations, including
/// the final outcome, duration, token usage, and per-iteration failure
/// details.
///
/// # Contract (Frozen)
/// - `outcome` is the final result (Validated, Failed, or BudgetExhausted)
/// - `validated_template` is `Some` only when outcome is Validated
/// - `failure_history` contains per-iteration failure details regardless of outcome
/// - `reusable_prompt` is `Some` only when outcome is Validated
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationReport {
    /// Globally unique execution identifier for this validation session.
    pub execution_id: Uuid,

    /// The final outcome of the validation loop.
    pub outcome: ValidationOutcome,

    /// Number of iterations executed.
    pub iterations: u32,

    /// Total wall-clock duration across all iterations (milliseconds).
    pub total_duration_ms: u64,

    /// Cumulative LLM tokens consumed across all iterations.
    pub cumulative_tokens: u64,

    /// Per-iteration failure details, ordered by iteration (1-indexed).
    #[serde(default)]
    pub failure_history: Vec<ValidationIterationReport>,

    /// The final validated template (present only if outcome is Validated).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub validated_template: Option<Template>,

    /// The validated llm_generate prompt (reusable for future executions).
    ///
    /// When a template is validated, the llm_generate prompt has been
    /// refined through the validation loop and can be reused without
    /// re-running validation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reusable_prompt: Option<String>,

    /// ISO 8601 timestamp when the report was created.
    pub created_at: DateTime<Utc>,
}

impl ValidationReport {
    /// Create a new validation report for the given execution.
    ///
    /// Initialises with default values (outcome not set, 0 iterations).
    /// Use builder methods to fill in the details as the loop progresses.
    pub fn new(execution_id: Uuid) -> Self {
        Self {
            execution_id,
            outcome: ValidationOutcome::Failed,
            iterations: 0,
            total_duration_ms: 0,
            cumulative_tokens: 0,
            failure_history: Vec::new(),
            validated_template: None,
            reusable_prompt: None,
            created_at: Utc::now(),
        }
    }

    /// Set the final outcome of the validation loop.
    pub fn with_outcome(mut self, outcome: ValidationOutcome) -> Self {
        self.outcome = outcome;
        self
    }

    /// Set the number of iterations executed.
    pub fn with_iterations(mut self, iterations: u32) -> Self {
        self.iterations = iterations;
        self
    }

    /// Set the total duration in milliseconds.
    pub fn with_duration(mut self, duration_ms: u64) -> Self {
        self.total_duration_ms = duration_ms;
        self
    }

    /// Set the cumulative token usage.
    pub fn with_cumulative_tokens(mut self, tokens: u64) -> Self {
        self.cumulative_tokens = tokens;
        self
    }

    /// Add a per-iteration report to the history.
    pub fn with_iteration(mut self, iteration: ValidationIterationReport) -> Self {
        self.failure_history.push(iteration);
        self
    }

    /// Set the validated template.
    pub fn with_validated_template(mut self, template: Template) -> Self {
        self.validated_template = Some(template);
        self
    }

    /// Set the reusable prompt.
    pub fn with_reusable_prompt(mut self, prompt: String) -> Self {
        self.reusable_prompt = Some(prompt);
        self
    }

    /// Returns `true` if the validation was successful.
    pub fn is_successful(&self) -> bool {
        self.outcome == ValidationOutcome::Validated
    }

    /// Returns `true` if the outcome was budget exhaustion.
    pub fn is_budget_exhausted(&self) -> bool {
        self.outcome == ValidationOutcome::BudgetExhausted
    }

    /// Get the total number of failures across all iterations.
    pub fn total_failures(&self) -> usize {
        self.failure_history
            .iter()
            .map(|r| r.failures.len())
            .sum()
    }
}

/// Per-iteration details within a validation report.
///
/// Captures what happened in a single iteration: which failures
/// occurred, how many tokens were consumed, how long it took,
/// and what fixes were applied from the previous iteration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationIterationReport {
    /// The 1-indexed iteration number.
    pub iteration: u32,

    /// Failures encountered in this iteration.
    #[serde(default)]
    pub failures: Vec<TemplateFailure>,

    /// LLM tokens consumed in this iteration.
    pub llm_tokens_used: u64,

    /// Wall-clock duration of this iteration (milliseconds).
    pub duration_ms: u64,

    /// What was fixed from the previous iteration (human-readable).
    #[serde(default)]
    pub fixes_applied: Vec<String>,

    /// Whether this iteration passed validation.
    pub passed: bool,
}

impl ValidationIterationReport {
    /// Create a new per-iteration report.
    pub fn new(iteration: u32) -> Self {
        Self {
            iteration,
            failures: Vec::new(),
            llm_tokens_used: 0,
            duration_ms: 0,
            fixes_applied: Vec::new(),
            passed: false,
        }
    }

    /// Record failures for this iteration.
    pub fn with_failures(mut self, failures: Vec<TemplateFailure>) -> Self {
        self.failures = failures;
        self
    }

    /// Set the LLM tokens consumed in this iteration.
    pub fn with_tokens(mut self, tokens: u64) -> Self {
        self.llm_tokens_used = tokens;
        self
    }

    /// Set the duration of this iteration.
    pub fn with_duration(mut self, duration_ms: u64) -> Self {
        self.duration_ms = duration_ms;
        self
    }

    /// Record fixes applied from the previous iteration.
    pub fn with_fixes(mut self, fixes: Vec<String>) -> Self {
        self.fixes_applied = fixes;
        self
    }

    /// Mark this iteration as passed.
    pub fn mark_passed(mut self) -> Self {
        self.passed = true;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::failure_parser::domain::failure::{SourceLocation, TemplateFailure};

    fn sample_failure() -> TemplateFailure {
        TemplateFailure::MissingSymbol {
            symbol: "addTask".into(),
            available: vec!["add".into()],
            suggestion: Some("use 'add'".into()),
            location: SourceLocation::new("test.ts", 10, None),
        }
    }

    #[test]
    fn test_new_report() {
        let report = ValidationReport::new(Uuid::new_v4());
        assert_eq!(report.iterations, 0);
        assert_eq!(report.outcome, ValidationOutcome::Failed);
        assert!(report.validated_template.is_none());
    }

    #[test]
    fn test_builder_methods() {
        let report = ValidationReport::new(Uuid::new_v4())
            .with_outcome(ValidationOutcome::Validated)
            .with_iterations(2)
            .with_duration(1500)
            .with_cumulative_tokens(10_000);

        assert!(report.is_successful());
        assert_eq!(report.iterations, 2);
        assert_eq!(report.total_duration_ms, 1500);
        assert_eq!(report.cumulative_tokens, 10_000);
    }

    #[test]
    fn test_with_iteration() {
        let iter_report = ValidationIterationReport::new(1)
            .with_failures(vec![sample_failure()])
            .with_tokens(5000)
            .with_duration(800);

        let report = ValidationReport::new(Uuid::new_v4())
            .with_iteration(iter_report);

        assert_eq!(report.failure_history.len(), 1);
        assert_eq!(report.total_failures(), 1);
    }

    #[test]
    fn test_is_successful() {
        let report = ValidationReport::new(Uuid::new_v4());
        assert!(!report.is_successful());

        let report = report.with_outcome(ValidationOutcome::Validated);
        assert!(report.is_successful());
    }

    #[test]
    fn test_is_budget_exhausted() {
        let report = ValidationReport::new(Uuid::new_v4())
            .with_outcome(ValidationOutcome::BudgetExhausted);
        assert!(report.is_budget_exhausted());
    }

    #[test]
    fn test_iteration_report() {
        let iter = ValidationIterationReport::new(1)
            .with_failures(vec![sample_failure()])
            .with_tokens(2000)
            .with_duration(500)
            .with_fixes(vec!["Fixed missing symbol addTask".into()])
            .mark_passed();

        assert_eq!(iter.iteration, 1);
        assert_eq!(iter.llm_tokens_used, 2000);
        assert_eq!(iter.duration_ms, 500);
        assert_eq!(iter.fixes_applied.len(), 1);
        assert!(iter.passed);
    }
}
