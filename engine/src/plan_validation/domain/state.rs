//! ValidationState — mutable per-iteration state for the validation loop.
//!
//! @canonical .pi/architecture/modules/plan-validation.md#state
//! Implements: Contract Freeze — ValidationState
//! Issue: issue-contract-freeze
//!
//! Tracks the current iteration, the augmented intent, failure history,
//! the template being validated, cumulative token usage, and success
//! status. This state is maintained across retry iterations so that
//! each attempt is informed by all previous failures.
//!
//! # Contract (Frozen)
//! - All fields are public for direct access by the validation loop
//! - State is cloned between iterations (immutable history preserved)
//! - No framework-specific dependencies
//! - Serialization support for persistence and event payloads

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::failure_parser::domain::TemplateFailure;
use crate::planning::domain::intent::UserIntent;
use crate::templates::domain::Template;

/// Per-iteration state for the plan validation loop.
///
/// Maintained across all retry iterations. Each validation attempt
/// adds its failures to the `failure_history` and may update the
/// `current_intent` via `ContextAugmenter::augment_intent()`.
///
/// # Lifespan
///
/// Created at the start of `ValidationLoopService::validate()` and
/// updated after each failed iteration. Discarded or archived in
/// the `ValidationReport` when the loop completes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationState {
    /// Globally unique execution identifier for this validation session.
    pub execution_id: Uuid,

    /// Current iteration number (1-indexed).
    ///
    /// Starts at 1 (first attempt). Incremented after each failed
    /// iteration before retrying.
    pub iteration: u32,

    /// The current augmented intent (original intent + failure context).
    ///
    /// Updated by `ContextAugmenter::augment_intent()` after each
    /// failed iteration to include structured failure feedback for
    /// the LLM to self-correct.
    pub current_intent: UserIntent,

    /// All failures encountered so far, ordered by iteration.
    ///
    /// Outer vec: iteration index (0-based).
    /// Inner vec: failures within that iteration.
    /// Example: `failure_history[0]` = failures from iteration 1.
    pub failure_history: Vec<Vec<TemplateFailure>>,

    /// The template being validated (if generated).
    ///
    /// Set after the first plan+execute attempt. Updated on retry
    /// when generative nodes produce corrected output.
    pub template: Option<Template>,

    /// Cumulative LLM token usage across all iterations.
    pub cumulative_tokens: u64,

    /// Whether the validation has succeeded.
    ///
    /// Set to `true` when `required_quality` has been met. Once true,
    /// the loop exits.
    pub succeeded: bool,
}

impl ValidationState {
    /// Create a new validation state for the first iteration.
    ///
    /// # Arguments
    ///
    /// * `execution_id` — Unique ID for this validation session.
    /// * `intent` — The original user intent (will be augmented on failure).
    pub fn new(execution_id: Uuid, intent: UserIntent) -> Self {
        Self {
            execution_id,
            iteration: 1,
            current_intent: intent,
            failure_history: Vec::new(),
            template: None,
            cumulative_tokens: 0,
            succeeded: false,
        }
    }

    /// Record failures for the current iteration and increment iteration count.
    ///
    /// Called after a failed validation attempt. Stores the failures
    /// for the current iteration and advances the counter.
    ///
    /// # Arguments
    ///
    /// * `failures` — The failures from the current iteration.
    /// * `tokens_used` — LLM tokens consumed in this iteration.
    pub fn record_failure(&mut self, failures: Vec<TemplateFailure>, tokens_used: u64) {
        self.failure_history.push(failures);
        self.cumulative_tokens += tokens_used;
        self.iteration += 1;
    }

    /// Get the failures from the most recent iteration, if any.
    pub fn latest_failures(&self) -> Option<&[TemplateFailure]> {
        self.failure_history.last().map(|v| v.as_slice())
    }

    /// Get the total number of failures across all iterations.
    pub fn total_failures(&self) -> usize {
        self.failure_history.iter().map(|v| v.len()).sum()
    }

    /// Get the number of attempted iterations so far.
    pub fn attempts(&self) -> u32 {
        self.iteration.saturating_sub(1)
    }

    /// Check if there have been any failures recorded.
    pub fn has_failures(&self) -> bool {
        self.failure_history.iter().any(|v| !v.is_empty())
    }

    /// Update the current template being validated.
    pub fn set_template(&mut self, template: Template) {
        self.template = Some(template);
    }

    /// Mark the validation as succeeded.
    pub fn mark_succeeded(&mut self) {
        self.succeeded = true;
    }

    /// Add LLM tokens to the cumulative count.
    pub fn add_tokens(&mut self, tokens: u64) {
        self.cumulative_tokens = self.cumulative_tokens.saturating_add(tokens);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::failure_parser::domain::failure::SourceLocation;

    fn sample_failure() -> TemplateFailure {
        TemplateFailure::MissingSymbol {
            symbol: "addTask".into(),
            available: vec!["add".into(), "remove".into()],
            suggestion: Some("use 'add' instead".into()),
            location: SourceLocation::new("test.ts", 10, None),
        }
    }

    fn sample_intent() -> UserIntent {
        UserIntent::new("Add a new method to TaskList".into(), None)
    }

    #[test]
    fn test_new_state() {
        let state = ValidationState::new(Uuid::new_v4(), sample_intent());
        assert_eq!(state.iteration, 1);
        assert!(!state.succeeded);
        assert!(state.failure_history.is_empty());
        assert!(state.template.is_none());
        assert_eq!(state.cumulative_tokens, 0);
    }

    #[test]
    fn test_record_failure() {
        let mut state = ValidationState::new(Uuid::new_v4(), sample_intent());
        let failures = vec![sample_failure()];
        state.record_failure(failures, 1000);

        assert_eq!(state.iteration, 2);
        assert_eq!(state.cumulative_tokens, 1000);
        assert_eq!(state.failure_history.len(), 1);
        assert_eq!(state.failure_history[0].len(), 1);
    }

    #[test]
    fn test_latest_failures() {
        let mut state = ValidationState::new(Uuid::new_v4(), sample_intent());
        assert!(state.latest_failures().is_none());

        state.record_failure(vec![sample_failure()], 500);
        assert!(state.latest_failures().is_some());
        assert_eq!(state.latest_failures().unwrap().len(), 1);
    }

    #[test]
    fn test_total_failures() {
        let mut state = ValidationState::new(Uuid::new_v4(), sample_intent());
        assert_eq!(state.total_failures(), 0);

        state.record_failure(vec![sample_failure(), sample_failure()], 500);
        assert_eq!(state.total_failures(), 2);

        state.record_failure(vec![sample_failure()], 300);
        assert_eq!(state.total_failures(), 3);
    }

    #[test]
    fn test_attempts() {
        let mut state = ValidationState::new(Uuid::new_v4(), sample_intent());
        assert_eq!(state.attempts(), 0);

        state.record_failure(vec![], 0);
        assert_eq!(state.attempts(), 1);
    }

    #[test]
    fn test_set_template() {
        let mut state = ValidationState::new(Uuid::new_v4(), sample_intent());
        assert!(state.template.is_none());

        let template = Template {
            id: "test-template".into(),
            ..Default::default()
        };
        state.set_template(template);
        assert!(state.template.is_some());
    }

    #[test]
    fn test_mark_succeeded() {
        let mut state = ValidationState::new(Uuid::new_v4(), sample_intent());
        assert!(!state.succeeded);
        state.mark_succeeded();
        assert!(state.succeeded);
    }

    #[test]
    fn test_add_tokens() {
        let mut state = ValidationState::new(Uuid::new_v4(), sample_intent());
        state.add_tokens(500);
        assert_eq!(state.cumulative_tokens, 500);

        state.add_tokens(300);
        assert_eq!(state.cumulative_tokens, 800);
    }

    #[test]
    fn test_has_failures() {
        let mut state = ValidationState::new(Uuid::new_v4(), sample_intent());
        assert!(!state.has_failures());

        state.record_failure(vec![sample_failure()], 500);
        assert!(state.has_failures());
    }
}
