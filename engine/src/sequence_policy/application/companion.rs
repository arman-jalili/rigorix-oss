//! CompanionStepObligation — R3 required-companion-step obligation, closing the
//! ADR-015 non-goal.
//!
//! @canonical .pi/architecture/modules/precondition.md#r3
//! @canonical .pi/architecture/modules/sequence-policy.md#r9--operator-controlled-step-requirements-adr-015
//! @canonical .pi/architecture/decisions/ADR-017-consequence-gating.md
//! @canonical .pi/architecture/decisions/ADR-015-operator-step-requirements.md
//! Implements: Contract Freeze — CompanionStepObligation + CompanionFinding
//! Issue: #938 (consequence-gating epic — contract freeze); behavior closed in
//!   ISSUE-CONSEQUENCE-GATING-9 (CompanionStepObligation); relocated into
//!   `sequence_policy` as an ADR-015 `[[requirements]]` extension by
//!   ISSUE-PF-REL-2 (#973)
//!
//! ADR-015 listed the **required-companion-step** obligation ("a matched step
//! is only allowed if the plan also contains a step matching R") as a follow-up.
//! It is a requirement, not a follow-up: a consequential step must be able to
//! require that its check is present in the same plan.
//!
//! It is an extension of the ADR-015 `[[requirements]]` surface:
//!
//! ```toml
//! [[requirements]]
//! id = "payout-needs-recheck"
//! match = { tool = "payment_execute" }
//! require_companion_step = { tool = "authority_recheck" }
//! action = "deny"        # deny (default) | promote
//! ```
//!
//! # Contract (Frozen)
//! - `match` reuses the frozen
//!   [`StepPredicate`](crate::sequence_policy::domain::StepPredicate) matcher
//! - `require_companion_step` is itself a `StepPredicate`; unmet (no step in the
//!   plan matches) → `action`
//! - `action` defaults to `deny`; `promote` sets `requires_approval = true` on
//!   the matched step and reuses the ADR-011 pause/resume chain
//! - A [`CompanionFinding`] records ids and step names only — parameter VALUES
//!   never appear (SpanPrivacy)
//! - Companion findings are projected into [`RequirementFinding`] so they reuse
//!   the ADR-015 `requirement_findings[]` evidence path

use serde::{Deserialize, Serialize};

use crate::sequence_policy::domain::{
    RequirementAction, RequirementFinding, SequencePolicyError, StepPredicate, StepRequirement,
};

use super::dto::PlannedStep;

/// Action taken when a required companion step is absent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CompanionAction {
    /// Refuse the plan / matched step. Default.
    #[default]
    Deny,
    /// Set `requires_approval = true` on the matched step — the existing
    /// approval pause/resume chain decides.
    Promote,
}

/// One operator-authored required-companion-step obligation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompanionStepObligation {
    /// Stable identifier, e.g. `"payout-needs-recheck"`.
    pub id: String,
    /// Selects the steps the obligation governs.
    #[serde(rename = "match")]
    pub r#match: StepPredicate,
    /// The predicate a companion step in the same plan MUST satisfy.
    pub require_companion_step: StepPredicate,
    /// Action when the companion is absent: `deny` (default) or `promote`.
    #[serde(default)]
    pub action: CompanionAction,
}

impl CompanionStepObligation {
    /// Build the companion obligation from an ADR-015 [`StepRequirement`] that
    /// declares `require_companion_step`. `None` when the requirement has no
    /// companion obligation.
    pub fn from_requirement(requirement: &StepRequirement) -> Option<Self> {
        requirement
            .require_companion_step
            .clone()
            .map(|require_companion_step| Self {
                id: requirement.id.clone(),
                r#match: requirement.r#match.clone(),
                require_companion_step,
                action: match requirement.action {
                    RequirementAction::Deny => CompanionAction::Deny,
                    RequirementAction::Promote => CompanionAction::Promote,
                },
            })
    }
}

/// A recorded, redacted companion-step outcome for one matched step.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompanionFinding {
    /// Stable id of the obligation that fired.
    pub obligation_id: String,
    /// Name of the matched step missing its companion.
    pub step: String,
    /// Name of the companion step that was required (its tool pattern).
    pub required_companion: String,
    /// Effective action: `deny` or `promote`.
    pub action: CompanionAction,
}

impl CompanionFinding {
    /// Redacted one-line summary — parameter values are never included.
    pub fn decision_summary(&self) -> String {
        format!(
            "companion-step obligation '{}' {} step '{}' (required companion '{}')",
            self.obligation_id,
            match self.action {
                CompanionAction::Deny => "denied",
                CompanionAction::Promote => "promoted",
            },
            self.step,
            self.required_companion
        )
    }

    /// Project into the ADR-015 [`RequirementFinding`] so companion refusals
    /// are carried in the shared `requirement_findings[]` evidence array.
    pub fn to_requirement_finding(&self) -> RequirementFinding {
        RequirementFinding {
            requirement_id: self.obligation_id.clone(),
            requirement_name: format!("companion-step obligation '{}'", self.obligation_id),
            step: self.step.clone(),
            unmet_identity: false,
            unmet_params: Vec::new(),
            unmet_companion: Some(self.required_companion.clone()),
            action: match self.action {
                CompanionAction::Deny => RequirementAction::Deny,
                CompanionAction::Promote => RequirementAction::Promote,
            },
        }
    }
}

/// Application service for R3 companion-step obligations.
#[async_trait::async_trait]
pub trait CompanionStepObligationService: Send + Sync {
    /// Evaluate the obligations over a fully-materialized ordered plan.
    ///
    /// Returns one [`CompanionFinding`] per matched step whose companion is
    /// absent, in deterministic order (obligation config order, then step
    /// order). An empty result means every matched step had its companion.
    ///
    /// # Errors
    /// - [`SequencePolicyError::InvalidConfig`] — a corrupt obligation config
    ///   → fail closed
    async fn evaluate_plan(
        &self,
        steps: &[PlannedStep],
    ) -> Result<Vec<CompanionFinding>, SequencePolicyError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sequence_policy::domain::StepPredicate;

    fn obligation() -> CompanionStepObligation {
        CompanionStepObligation {
            id: "payout-needs-recheck".to_string(),
            r#match: StepPredicate {
                tool: "payment_execute".to_string(),
                params: vec![],
            },
            require_companion_step: StepPredicate {
                tool: "authority_recheck".to_string(),
                params: vec![],
            },
            action: CompanionAction::Deny,
        }
    }

    #[test]
    fn action_defaults_to_deny() {
        let parsed: CompanionStepObligation = serde_json::from_value(serde_json::json!({
            "id": "payout-needs-recheck",
            "match": { "tool": "payment_execute" },
            "require_companion_step": { "tool": "authority_recheck" }
        }))
        .expect("minimal obligation parses");
        assert_eq!(parsed.action, CompanionAction::Deny);
    }

    #[test]
    fn finding_summary_is_redacted() {
        let finding = CompanionFinding {
            obligation_id: obligation().id,
            step: "pay".to_string(),
            required_companion: "authority_recheck".to_string(),
            action: CompanionAction::Deny,
        };
        let summary = finding.decision_summary();
        assert!(summary.contains("payout-needs-recheck"));
        assert!(summary.contains("authority_recheck"));
    }

    #[test]
    fn finding_projects_into_requirement_finding() {
        let finding = CompanionFinding {
            obligation_id: "payout-needs-recheck".to_string(),
            step: "pay".to_string(),
            required_companion: "authority_recheck".to_string(),
            action: CompanionAction::Promote,
        };
        let requirement = finding.to_requirement_finding();
        assert_eq!(requirement.requirement_id, "payout-needs-recheck");
        assert_eq!(
            requirement.unmet_companion.as_deref(),
            Some("authority_recheck")
        );
        assert_eq!(requirement.action, RequirementAction::Promote);
        assert!(requirement.decision_summary().contains("authority_recheck"));
    }
}
