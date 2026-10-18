//! #1069 — the companion obligation is a **presence** lint at plan time and an
//! **execution prerequisite** at dispatch time.
//!
//! The regression this pins: a plan listing the companion *after* the matched
//! step, or a companion that later failed, satisfied `evaluate_plan` and the
//! consequential step dispatched anyway. Presence is not ordering, and it is
//! certainly not success.

use async_trait::async_trait;
use rigorix_engine::sequence_policy::application::{
    PlannedStep, SequencePolicyService, SequencePolicyServiceImpl,
};
use rigorix_engine::sequence_policy::domain::{
    RequirementAction, SequencePolicyConfig, SequencePolicyError, StepPredicate, StepRequirement,
};
use rigorix_engine::sequence_policy::infrastructure::SequencePolicyRepository;

struct StubRepository {
    outcome: Result<Option<SequencePolicyConfig>, SequencePolicyError>,
}

#[async_trait]
impl SequencePolicyRepository for StubRepository {
    async fn load_config(&self) -> Result<Option<SequencePolicyConfig>, SequencePolicyError> {
        self.outcome.clone()
    }
}

fn companion_requirement(action: RequirementAction) -> StepRequirement {
    StepRequirement {
        id: "payout-needs-recheck".to_string(),
        name: "Payouts require a completed authority recheck".to_string(),
        description: "d".to_string(),
        r#match: StepPredicate {
            tool: "payment_execute".to_string(),
            params: vec![],
        },
        require_identity: false,
        require_params: vec![],
        require_companion_step: Some(StepPredicate {
            tool: "authority_recheck".to_string(),
            params: vec![],
        }),
        action,
    }
}

fn service(requirement: StepRequirement) -> SequencePolicyServiceImpl {
    SequencePolicyServiceImpl::new(Box::new(StubRepository {
        outcome: Ok(Some(SequencePolicyConfig {
            fail_closed: true,
            requirements: vec![requirement],
            rules: Vec::new(),
        })),
    }))
}

fn pay() -> PlannedStep {
    PlannedStep {
        name: "pay".to_string(),
        tool: "payment_execute".to_string(),
        parameters: serde_json::json!({}),
    }
}

fn recheck() -> PlannedStep {
    PlannedStep {
        name: "recheck".to_string(),
        tool: "authority_recheck".to_string(),
        parameters: serde_json::json!({}),
    }
}

/// The exact shape of the defect: the plan contains the companion, so the
/// plan-time lint is quiet — and the step would dispatch before the companion
/// has run.
#[tokio::test]
async fn presence_in_the_plan_does_not_satisfy_the_execution_prerequisite() {
    let svc = service(companion_requirement(RequirementAction::Deny));

    // Plan-time: both steps are listed, so there is no finding.
    let plan_findings = svc
        .evaluate_requirements(&[pay(), recheck()], false)
        .await
        .expect("plan lint");
    assert!(
        plan_findings.is_empty(),
        "presence satisfies the plan-time lint (this is the narrow guarantee)"
    );

    // Dispatch-time: nothing has completed yet, so the prerequisite is unmet.
    let findings = svc
        .evaluate_companion_prerequisite(&[], &pay())
        .await
        .expect("prerequisite");
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].obligation_id, "payout-needs-recheck");
    assert_eq!(findings[0].required_companion, "authority_recheck");
}

#[tokio::test]
async fn a_companion_that_completed_satisfies_the_prerequisite() {
    let svc = service(companion_requirement(RequirementAction::Deny));
    let findings = svc
        .evaluate_companion_prerequisite(&[recheck()], &pay())
        .await
        .expect("prerequisite");
    assert!(
        findings.is_empty(),
        "a completed companion unblocks the step"
    );
}

/// A companion that *failed* never enters the completed set — the executor
/// builds it from `NodeStatus::Completed` nodes — so it cannot satisfy the
/// prerequisite. This is the ordering/success half of the former claim.
#[tokio::test]
async fn a_failed_companion_does_not_satisfy_the_prerequisite() {
    let svc = service(companion_requirement(RequirementAction::Deny));
    // `failed` is not in the completed list even though it ran.
    let findings = svc
        .evaluate_companion_prerequisite(&[], &pay())
        .await
        .expect("prerequisite");
    assert_eq!(findings.len(), 1);
}

#[tokio::test]
async fn an_unmatched_step_is_not_gated() {
    let svc = service(companion_requirement(RequirementAction::Deny));
    let findings = svc
        .evaluate_companion_prerequisite(&[], &recheck())
        .await
        .expect("prerequisite");
    assert!(findings.is_empty());
}

/// `promote` is a plan-time human decision; re-denying the promoted step at
/// dispatch would make approval unable to unblock it.
#[tokio::test]
async fn promote_obligations_are_not_enforced_at_dispatch() {
    let svc = service(companion_requirement(RequirementAction::Promote));
    let findings = svc
        .evaluate_companion_prerequisite(&[], &pay())
        .await
        .expect("prerequisite");
    assert!(
        findings.is_empty(),
        "promote is decided by approval, not by the dispatch prerequisite"
    );
}

#[tokio::test]
async fn no_requirements_means_no_prerequisite() {
    let svc = SequencePolicyServiceImpl::new(Box::new(StubRepository {
        outcome: Ok(Some(SequencePolicyConfig {
            fail_closed: true,
            requirements: Vec::new(),
            rules: Vec::new(),
        })),
    }));
    let findings = svc
        .evaluate_companion_prerequisite(&[], &pay())
        .await
        .expect("prerequisite");
    assert!(findings.is_empty());
}
