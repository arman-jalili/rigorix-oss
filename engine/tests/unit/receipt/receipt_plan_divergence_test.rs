//! A receipt describes the plan, and the plan is what runs (#1588).
//!
//! Until this existed the two were never compared. The steps that execute come from the plan; the
//! declared effects an attestation measures `undeclared_writes` against come from the receipt. A
//! receipt naming a different set therefore hands the auditor a declaration about a run that did not
//! happen — and a step the plan gates behind an approval can be approved under a declaration that
//! does not mention it.
//!
//! Every case here is a divergence the comparison must refuse, and each names its own variant with
//! the position, because "the sets differ" is not a finding an author can act on.

use rigorix_engine::receipt::domain::receipt::{
    BudgetDeclared, DeclaredEffect, Expectation, Receipt, ReceiptStep, Role, Sandbox,
    SandboxRegime, Workspace, WorkspaceKind,
};
use rigorix_engine::receipt::domain::refusal::ReceiptRefusal;
use rigorix_engine::receipt::{PlanStep, verify_against_plan};
use uuid::Uuid;

fn step(id: &str, requires_approval: bool) -> ReceiptStep {
    ReceiptStep {
        id: id.to_string(),
        tool: "run_command".to_string(),
        parameters: serde_json::json!({ "command": "true" }),
        depends_on: Vec::new(),
        declared_effects: DeclaredEffect {
            paths: vec![format!("{id}.txt")],
            content_digests: vec![format!("digest-of-{id}")],
        },
        expectation: Expectation {
            description: format!("{id} writes {id}.txt"),
            quality_level: None,
        },
        requires_approval,
        timeout_secs: None,
    }
}

fn receipt(steps: Vec<ReceiptStep>) -> Receipt {
    Receipt::new(
        Uuid::nil(),
        Workspace {
            kind: WorkspaceKind::CommitTree,
            value: "tree".to_string(),
        },
        Sandbox {
            regime: SandboxRegime::Container,
            image_digest: "sha256:1111".to_string(),
        },
        steps,
        vec![Role {
            role: "executor".to_string(),
            model_id: "m".to_string(),
            context_window: 8192,
            structured_output: false,
        }],
        BudgetDeclared {
            max_iterations: 10,
            max_tokens: 1000,
        },
        rigorix_engine::quality_gates::domain::level::QualityLevel::MergeReady,
    )
    .expect("the fixture carries no credentials")
}

fn plan(names: &[(&str, bool)]) -> Vec<PlanStep> {
    names
        .iter()
        .map(|(name, approval)| PlanStep::new(*name, *approval))
        .collect()
}

#[test]
fn a_receipt_that_matches_the_plan_is_accepted() {
    let receipt = receipt(vec![step("write", false), step("verify", true)]);
    let plan = plan(&[("write", false), ("verify", true)]);

    assert!(verify_against_plan(&receipt, &plan).is_ok());
}

#[test]
fn a_step_missing_from_the_receipt_is_refused_and_named() {
    // The subset case: the plan has two steps, the receipt declares one. What would run is the plan;
    // what the auditor would read is the receipt — one step short and silent about it.
    let receipt = receipt(vec![step("write", false)]);
    let plan = plan(&[("write", false), ("verify", false)]);

    let err = verify_against_plan(&receipt, &plan).expect_err("a strict subset is a divergence");
    assert_eq!(
        err,
        vec![ReceiptRefusal::PlanStepNotDeclared {
            step: "verify".to_string(),
            index: 1,
        }]
    );
    assert_eq!(err[0].kind(), "plan_step_not_declared");
    assert!(format!("{}", err[0]).contains("verify"), "{err:?}");
}

#[test]
fn a_step_the_plan_does_not_have_is_refused_and_named() {
    let receipt = receipt(vec![step("write", false), step("exfiltrate", false)]);
    let plan = plan(&[("write", false)]);

    let err = verify_against_plan(&receipt, &plan).expect_err("an undeclared step is a divergence");
    assert_eq!(
        err,
        vec![ReceiptRefusal::StepNotInPlan {
            step: "exfiltrate".to_string(),
            index: 1,
        }]
    );
    assert_eq!(err[0].kind(), "step_not_in_plan");
}

#[test]
fn a_reordering_is_refused_even_though_the_names_are_the_same() {
    // Positional, not set-based: order decides which step runs first against a workspace whose tree
    // was pinned for that order, so a swap is a different runbook.
    let receipt = receipt(vec![step("verify", false), step("write", false)]);
    let plan = plan(&[("write", false), ("verify", false)]);

    let err = verify_against_plan(&receipt, &plan).expect_err("a reordering is a divergence");
    assert_eq!(err.len(), 2, "both sides of the first divergence are named");
    assert_eq!(err[0].kind(), "step_not_in_plan");
    assert_eq!(err[1].kind(), "plan_step_not_declared");
}

#[test]
fn a_flipped_approval_requirement_is_refused_with_its_own_variant() {
    // The direction that matters: the plan gates the step, the receipt says it does not need one.
    let receipt = receipt(vec![step("migrate", false)]);
    let plan = plan(&[("migrate", true)]);

    let err =
        verify_against_plan(&receipt, &plan).expect_err("the plan's gate is not the receipt's");
    assert_eq!(
        err,
        vec![ReceiptRefusal::ApprovalRequirementMismatch {
            step: "migrate".to_string(),
            plan: true,
            receipt: false,
        }]
    );
    assert_eq!(err[0].kind(), "approval_requirement_mismatch");
}

#[test]
fn a_receipt_requiring_more_approval_than_the_plan_is_refused_too() {
    // The mirror: the receipt is stricter than the plan. Still a divergence — the two documents
    // describe different runbooks — and refusing it keeps `verify_against_plan` a comparison rather
    // than a one-way check with a preferred side.
    let receipt = receipt(vec![step("write", true)]);
    let plan = plan(&[("write", false)]);

    let err = verify_against_plan(&receipt, &plan).expect_err("both directions are divergences");
    assert_eq!(
        err,
        vec![ReceiptRefusal::ApprovalRequirementMismatch {
            step: "write".to_string(),
            plan: false,
            receipt: true,
        }]
    );
}

#[test]
fn a_mismatch_at_a_later_position_is_still_reported() {
    // The divergence is found where it is, not only at index 0 — and the positional report stops
    // there, because after the first divergence the two lists describe different runbooks.
    let receipt = receipt(vec![step("a", false), step("b", true)]);
    let plan = plan(&[("a", false), ("c", true)]);

    let err = verify_against_plan(&receipt, &plan).expect_err("two divergences");
    assert_eq!(err.len(), 2);
    assert_eq!(err[0].kind(), "step_not_in_plan");
    assert_eq!(err[1].kind(), "plan_step_not_declared");
}

#[test]
fn an_empty_receipt_against_a_plan_names_every_plan_step() {
    let receipt = receipt(vec![]);
    let plan = plan(&[("one", false), ("two", false)]);

    let err = verify_against_plan(&receipt, &plan).expect_err("an empty declaration");
    assert_eq!(err.len(), 2);
    assert!(err.iter().all(|r| r.kind() == "plan_step_not_declared"));
}
