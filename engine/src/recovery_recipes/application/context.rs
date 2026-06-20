//! RecoveryContext — per-session attempt tracker and recovery event log.
//!
//! @canonical .pi/architecture/modules/recovery-recipes.md#recoverycontext
//! Implements: Contract Freeze — RecoveryContext struct
//! Issue: #438 (recovery-recipes epic)
//!
//! # Contract (Frozen)
//! - Tracks per-scenario attempt counts within an execution session
//! - Maintains an ordered recovery event log for audit trail
//! - No framework dependencies — pure domain struct
//! - `can_attempt()` checks if a scenario has remaining attempts
//! - `record_attempt()` increments counter and emits events
//! - Constructed fresh per execution session

use std::collections::HashMap;

use crate::recovery_recipes::domain::{FailureScenario, RecoveryEvent, RecoveryRecipe};

/// Tracks per-scenario attempt counts and recovery events within an
/// execution session. Constructed fresh for each execution session
/// to ensure clean attempt tracking.
///
/// # Usage
///
/// ```ignore
/// let mut ctx = RecoveryContext::new();
///
/// if ctx.can_attempt(scenario, &recipe) {
///     // execute recovery steps
///     ctx.record_attempt(scenario);
/// } else {
///     // escalate
/// }
/// ```
#[derive(Debug, Clone)]
pub struct RecoveryContext {
    /// Per-scenario attempt count (reset per execution session).
    attempts: HashMap<FailureScenario, u32>,
    /// Ordered recovery event log.
    events: Vec<RecoveryEvent>,
}

impl RecoveryContext {
    /// Create a new, empty `RecoveryContext`.
    ///
    /// All scenario counters start at 0. No events have been recorded.
    pub fn new() -> Self {
        Self {
            attempts: HashMap::new(),
            events: Vec::new(),
        }
    }

    /// Check if a scenario has remaining attempts according to the recipe.
    ///
    /// Returns `true` if `current_attempts < recipe.max_attempts`.
    pub fn can_attempt(&self, scenario: FailureScenario, recipe: &RecoveryRecipe) -> bool {
        self.attempts.get(&scenario).copied().unwrap_or(0) < recipe.max_attempts
    }

    /// Record an attempt (increments counter, appends to event log).
    ///
    /// Returns the attempt number (1-based) that was recorded.
    pub fn record_attempt(&mut self, scenario: FailureScenario) -> u32 {
        let count = self.attempts.entry(scenario).or_insert(0);
        *count += 1;
        *count
    }

    /// Record a recovery event in the event log.
    pub fn record_event(&mut self, event: RecoveryEvent) {
        self.events.push(event);
    }

    /// Get the current attempt count for a scenario.
    ///
    /// Returns 0 if no attempts have been recorded for this scenario.
    pub fn attempt_count(&self, scenario: FailureScenario) -> u32 {
        self.attempts.get(&scenario).copied().unwrap_or(0)
    }

    /// Get the remaining attempts for a scenario given a recipe.
    pub fn remaining_attempts(&self, scenario: FailureScenario, recipe: &RecoveryRecipe) -> u32 {
        recipe
            .max_attempts
            .saturating_sub(self.attempt_count(scenario))
    }

    /// Get all events for audit trail.
    pub fn events(&self) -> &[RecoveryEvent] {
        &self.events
    }

    /// Consume the context and return the event log.
    pub fn into_events(self) -> Vec<RecoveryEvent> {
        self.events
    }

    /// Clear all attempt counters and events (reset for a new session).
    pub fn reset(&mut self) {
        self.attempts.clear();
        self.events.clear();
    }

    /// Returns `true` if any attempts have been recorded.
    pub fn has_attempts(&self) -> bool {
        !self.attempts.is_empty()
    }

    /// Returns `true` if any events have been recorded.
    pub fn has_events(&self) -> bool {
        !self.events.is_empty()
    }

    /// Returns the number of scenarios that have been attempted.
    pub fn attempted_scenario_count(&self) -> usize {
        self.attempts.len()
    }

    /// Returns the total number of attempts across all scenarios.
    pub fn total_attempts(&self) -> u32 {
        self.attempts.values().sum()
    }
}

impl Default for RecoveryContext {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recovery_recipes::domain::EscalationPolicy;

    #[test]
    fn test_new_context_is_empty() {
        let ctx = RecoveryContext::new();
        assert_eq!(ctx.attempt_count(FailureScenario::CompileError), 0);
        assert!(!ctx.has_attempts());
        assert!(!ctx.has_events());
        assert_eq!(ctx.attempted_scenario_count(), 0);
        assert_eq!(ctx.total_attempts(), 0);
    }

    #[test]
    fn test_record_attempt_increments_counter() {
        let mut ctx = RecoveryContext::new();
        let count = ctx.record_attempt(FailureScenario::CompileError);
        assert_eq!(count, 1);
        assert_eq!(ctx.attempt_count(FailureScenario::CompileError), 1);
        assert_eq!(ctx.total_attempts(), 1);
        assert!(ctx.has_attempts());
    }

    #[test]
    fn test_multiple_attempts_same_scenario() {
        let mut ctx = RecoveryContext::new();
        ctx.record_attempt(FailureScenario::CompileError);
        ctx.record_attempt(FailureScenario::CompileError);
        assert_eq!(ctx.attempt_count(FailureScenario::CompileError), 2);
    }

    #[test]
    fn test_different_scenarios_tracked_independently() {
        let mut ctx = RecoveryContext::new();
        ctx.record_attempt(FailureScenario::CompileError);
        ctx.record_attempt(FailureScenario::TestFailure);
        ctx.record_attempt(FailureScenario::CompileError);

        assert_eq!(ctx.attempt_count(FailureScenario::CompileError), 2);
        assert_eq!(ctx.attempt_count(FailureScenario::TestFailure), 1);
        assert_eq!(ctx.total_attempts(), 3);
        assert_eq!(ctx.attempted_scenario_count(), 2);
    }

    #[test]
    fn test_can_attempt_within_limits() {
        let mut ctx = RecoveryContext::new();
        let recipe = RecoveryRecipe::new(
            FailureScenario::CompileError,
            vec![crate::recovery_recipes::domain::RecoveryStep::CleanBuild],
            2,
            EscalationPolicy::AlertHuman,
        )
        .unwrap();

        assert!(ctx.can_attempt(FailureScenario::CompileError, &recipe));
        ctx.record_attempt(FailureScenario::CompileError);
        assert!(ctx.can_attempt(FailureScenario::CompileError, &recipe));
        ctx.record_attempt(FailureScenario::CompileError);
        assert!(!ctx.can_attempt(FailureScenario::CompileError, &recipe));
    }

    #[test]
    fn test_remaining_attempts() {
        let mut ctx = RecoveryContext::new();
        let recipe = RecoveryRecipe::new(
            FailureScenario::CompileError,
            vec![crate::recovery_recipes::domain::RecoveryStep::CleanBuild],
            3,
            EscalationPolicy::AlertHuman,
        )
        .unwrap();

        assert_eq!(
            ctx.remaining_attempts(FailureScenario::CompileError, &recipe),
            3
        );
        ctx.record_attempt(FailureScenario::CompileError);
        assert_eq!(
            ctx.remaining_attempts(FailureScenario::CompileError, &recipe),
            2
        );
        ctx.record_attempt(FailureScenario::CompileError);
        assert_eq!(
            ctx.remaining_attempts(FailureScenario::CompileError, &recipe),
            1
        );
        ctx.record_attempt(FailureScenario::CompileError);
        assert_eq!(
            ctx.remaining_attempts(FailureScenario::CompileError, &recipe),
            0
        );
    }

    #[test]
    fn test_record_event() {
        let mut ctx = RecoveryContext::new();
        let event = RecoveryEvent::RecoveryAttempted {
            scenario: FailureScenario::CompileError,
            step: crate::recovery_recipes::domain::RecoveryStep::CleanBuild,
            attempt_number: 1,
        };
        ctx.record_event(event);
        assert!(ctx.has_events());
        assert_eq!(ctx.events().len(), 1);
    }

    #[test]
    fn test_reset() {
        let mut ctx = RecoveryContext::new();
        ctx.record_attempt(FailureScenario::CompileError);
        ctx.record_event(RecoveryEvent::RecoveryAttempted {
            scenario: FailureScenario::CompileError,
            step: crate::recovery_recipes::domain::RecoveryStep::CleanBuild,
            attempt_number: 1,
        });

        ctx.reset();
        assert!(!ctx.has_attempts());
        assert!(!ctx.has_events());
        assert_eq!(ctx.total_attempts(), 0);
    }

    #[test]
    fn test_into_events_consumes() {
        let mut ctx = RecoveryContext::new();
        ctx.record_event(RecoveryEvent::RecoveryAttempted {
            scenario: FailureScenario::CompileError,
            step: crate::recovery_recipes::domain::RecoveryStep::CleanBuild,
            attempt_number: 1,
        });
        let events = ctx.into_events();
        assert_eq!(events.len(), 1);
    }

    #[test]
    fn test_default_is_empty() {
        let ctx = RecoveryContext::default();
        assert_eq!(ctx.total_attempts(), 0);
    }

    #[test]
    fn test_unattempted_scenario_returns_zero() {
        let ctx = RecoveryContext::new();
        assert_eq!(ctx.attempt_count(FailureScenario::ProviderFailure), 0);
    }
}
