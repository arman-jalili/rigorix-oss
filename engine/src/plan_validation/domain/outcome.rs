//! ValidationOutcome — result of the validation loop execution.
//!
//! @canonical .pi/architecture/modules/plan-validation.md#outcome
//! Implements: Contract Freeze — ValidationOutcome
//! Issue: issue-contract-freeze
//!
//! Defines the possible outcomes of running a plan through the
//! validation loop: validated successfully, failed with retries
//! exhausted, or aborted due to budget exhaustion.
//!
//! # Contract (Frozen)
//! - Three mutually exclusive outcome variants
//! - Each variant carries structured context for downstream consumers
//! - Serialization support for API responses and event payloads
//! - No implementation logic

use serde::{Deserialize, Serialize};

/// The outcome of executing the plan validation loop.
///
/// Returned as part of `ValidationReport` to indicate whether the
/// template was successfully validated, failed after all retries
/// were exhausted, or hit the cumulative token budget.
///
/// # Contract (Frozen)
/// - Validated — template met the required quality level
/// - Failed — all retries exhausted, template did not meet quality
/// - BudgetExhausted — cumulative token budget consumed before validation
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ValidationOutcome {
    /// The template passed validation at the required quality level.
    ///
    /// The template is production-grade and can be cached as a
    /// reusable asset.
    Validated,

    /// All retry attempts exhausted without passing validation.
    ///
    /// The template could not be validated after all configured
    /// retries.
    Failed,

    /// The validation loop was aborted due to cumulative token budget exhaustion.
    ///
    /// The `max_cumulative_tokens` limit in `ValidationLoopConfig`
    /// was reached before validation could complete.
    BudgetExhausted,
}

impl ValidationOutcome {
    /// Returns `true` if the validation outcome is `Validated`.
    pub fn is_validated(&self) -> bool {
        matches!(self, ValidationOutcome::Validated)
    }

    /// Returns `true` if the validation outcome is `Failed`.
    pub fn is_failed(&self) -> bool {
        matches!(self, ValidationOutcome::Failed)
    }

    /// Returns `true` if the outcome was budget exhaustion.
    pub fn is_budget_exhausted(&self) -> bool {
        matches!(self, ValidationOutcome::BudgetExhausted)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validated_outcome() {
        let outcome = ValidationOutcome::Validated;
        assert!(outcome.is_validated());
        assert!(!outcome.is_failed());
        assert!(!outcome.is_budget_exhausted());
    }

    #[test]
    fn test_failed_outcome() {
        let outcome = ValidationOutcome::Failed;
        assert!(!outcome.is_validated());
        assert!(outcome.is_failed());
        assert!(!outcome.is_budget_exhausted());
    }

    #[test]
    fn test_budget_exhausted_outcome() {
        let outcome = ValidationOutcome::BudgetExhausted;
        assert!(!outcome.is_validated());
        assert!(!outcome.is_failed());
        assert!(outcome.is_budget_exhausted());
    }
}
