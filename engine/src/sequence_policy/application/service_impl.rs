//! SequencePolicyServiceImpl — concrete service skeleton.
//!
//! @canonical .pi/architecture/modules/sequence-policy.md#sequencepolicyservice
//! Implements: Contract Freeze — SequencePolicyServiceImpl stub
//! Issue: #838 (sequence-policy epic — contract freeze); behavior in
//!   ISSUE-SEQUENCE-POLICY-2 (evaluate_plan) and ISSUE-SEQUENCE-POLICY-5
//!   (windowed Matcher over compiled rules)
//!
//! The implementation loads the rule config per run through the injected
//! `SequencePolicyRepository`, compiles predicates once per load (glob/regex
//! compilation is a resource concern), and evaluates the ordered step list.
//! Method bodies are `todo!()` stubs — behavior lands in the implementation
//! issues.

use async_trait::async_trait;

use crate::sequence_policy::domain::{SequenceMatch, SequencePolicyError};

use super::dto::{DispatchedStep, PlannedStep};
use super::service::SequencePolicyService;
use crate::sequence_policy::infrastructure::repository::SequencePolicyRepository;

/// Default `SequencePolicyService` implementation.
///
/// # Construction
/// - `new(repository)` — inject the rule-config repository (filesystem-backed
///   `TomlSequencePolicyRepository`, or a signed-bundle seam for enterprise,
///   P3)
pub struct SequencePolicyServiceImpl {
    /// Rule-config repository — read by `evaluate_plan`/`evaluate_prefix` once
    /// their behavior lands (ISSUE-SEQUENCE-POLICY-2/-5).
    #[allow(dead_code)] // consumed by the upcoming evaluate_* implementations
    repository: Box<dyn SequencePolicyRepository>,
}

impl SequencePolicyServiceImpl {
    /// Create the service over the given rule-config repository.
    pub fn new(repository: Box<dyn SequencePolicyRepository>) -> Self {
        Self { repository }
    }
}

#[async_trait]
impl SequencePolicyService for SequencePolicyServiceImpl {
    async fn evaluate_plan(
        &self,
        _steps: &[PlannedStep],
    ) -> Result<Vec<SequenceMatch>, SequencePolicyError> {
        todo!(
            "ISSUE-SEQUENCE-POLICY-2/-5: load config via repository, match ordered steps, return matches"
        )
    }

    async fn evaluate_prefix(
        &self,
        _prefix: &[DispatchedStep],
        _next: &PlannedStep,
    ) -> Result<Vec<SequenceMatch>, SequencePolicyError> {
        todo!("ISSUE-SEQUENCE-POLICY-2/-5: evaluate completed prefix + next node, return matches")
    }
}
