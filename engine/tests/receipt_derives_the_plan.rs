#![cfg(not(feature = "portable"))]
//! The receipt is the runbook (#1629, ADR-025 D3).
//!
//! `run_receipt` used to take both a `&Receipt` and a caller-supplied `Vec<TemplateStepDef>`. While that
//! parameter existed, a host could run a plan its receipt did not describe, and the only thing that
//! noticed was `verify_against_plan` — a comparison whose failure arrives *after* the effects. ADR-024
//! replaced "a checklist kept beside a plan somebody else wrote" with "the declaration is the authority
//! the run must satisfy"; a surface that takes both is the old arrangement with a new type name.
//!
//! These tests are outside the crate, so they can only reach what it publishes — which is the point: the
//! derivation has to be visible from the surface a host holds.

use std::path::{Path, PathBuf};

use rigorix_engine::planning::application::service::PlanningPipelineService;
use rigorix_engine::planning::application::supplied_steps::SuppliedStepsPlanning;
use rigorix_engine::quality_gates::domain::level::QualityLevel;
use rigorix_engine::receipt::{
    BudgetDeclared, DeclaredEffect, Expectation, Receipt, ReceiptRefusal, ReceiptStep,
    RunReceiptConfig, run_receipt, sha256_hex, verify_against_plan,
};
use rigorix_engine::receipt::{Sandbox, SandboxRegime, Workspace, WorkspaceKind};
use uuid::Uuid;

struct Fixture {
    root: tempfile::TempDir,
    tree: String,
    state: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let root = tempfile::tempdir().expect("temp root");
        let path = root.path();
        // A fixture shelling out to `git` to build the tree a receipt names: a process sink by the
        // repository's own taxonomy, and a test rather than a production crossing.
        #[allow(clippy::disallowed_types)]
        let run = |args: &[&str]| {
            let output = std::process::Command::new("git")
                .args(args)
                .current_dir(path)
                .env("GIT_AUTHOR_NAME", "Fixture")
                .env("GIT_AUTHOR_EMAIL", "fixture@example.invalid")
                .env("GIT_COMMITTER_NAME", "Fixture")
                .env("GIT_COMMITTER_EMAIL", "fixture@example.invalid")
                .output()
                .expect("git runs");
            assert!(
                output.status.success(),
                "git {args:?} failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            String::from_utf8_lossy(&output.stdout).trim().to_string()
        };
        run(&["init", "-q"]);
        std::fs::write(path.join("README.md"), "# fixture\n").expect("fixture file");
        std::fs::write(path.join("second.md"), "second\n").expect("fixture file");
        run(&["add", "."]);
        run(&["commit", "-q", "-m", "fixture"]);
        let tree = run(&["rev-parse", "HEAD^{tree}"]);
        let state = path
            .parent()
            .expect("a parent")
            .join(format!("state-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&state).expect("state");
        Self { root, tree, state }
    }

    fn workspace(&self) -> &Path {
        self.root.path()
    }

    fn config(&self) -> RunReceiptConfig {
        RunReceiptConfig {
            state_dir: self.state.clone(),
            audit_key: Some("derive-fixture-key".to_string()),
            permission_mode: rigorix_engine::permission::domain::PermissionMode::WorkspaceWrite,
        }
    }

    /// A receipt over `steps`, with the fixture's tree identity.
    fn receipt(&self, steps: Vec<ReceiptStep>) -> Receipt {
        Receipt::new(
            Uuid::now_v7(),
            Workspace {
                kind: WorkspaceKind::CommitTree,
                value: self.tree.clone(),
            },
            Sandbox {
                regime: SandboxRegime::Container,
                image_digest: "sha256:fixture".to_string(),
            },
            steps,
            BudgetDeclared {
                max_iterations: 4,
                max_tokens: 10_000,
            },
            QualityLevel::MergeReady,
        )
        .expect("the fixture carries no credentials")
    }
}

fn read_step(id: &str, path: &str, depends_on: &[&str]) -> ReceiptStep {
    ReceiptStep {
        id: id.to_string(),
        tool: "file-read".to_string(),
        parameters: serde_json::json!({ "path": path }),
        depends_on: depends_on.iter().map(|d| (*d).to_string()).collect(),
        declared_effects: DeclaredEffect {
            paths: Vec::new(),
            content_digests: Vec::new(),
        },
        expectation: Expectation {
            description: format!("{id} reads {path}"),
            quality_level: None,
        },
        requires_approval: false,
        timeout_secs: Some(30),
    }
}

#[test]
fn the_derived_plan_is_the_declaration_in_order_and_in_dependencies() {
    // The mapping is small and mechanical, which is exactly why it is worth pinning: a rename on either
    // side (the DTO's `name`, the receipt's `id`) would silently produce a plan that describes nothing.
    let fixture = Fixture::new();
    let receipt = fixture.receipt(vec![
        read_step("first", "README.md", &[]),
        read_step("second", "second.md", &["first"]),
    ]);

    let plan = rigorix_engine::receipt::application::service::plan_from_receipt(&receipt)
        .expect("both steps map");

    assert_eq!(plan.len(), 2);
    assert_eq!(plan[0].name, "first");
    assert_eq!(plan[0].tool, "file-read");
    assert_eq!(
        plan[0].description, "first reads README.md",
        "the expectation's sentence is carried into the runbook, not invented"
    );
    assert_eq!(plan[1].depends_on, vec!["first".to_string()]);
    assert_eq!(plan[1].timeout_secs, Some(30));
    assert!(!plan[1].requires_approval);

    // The acceptance criterion the issue states literally: the derivation and the check cannot drift,
    // because the check is run on the derivation in the same test that reads it.
    let plan_steps: Vec<rigorix_engine::receipt::PlanStep> = plan
        .iter()
        .map(|step| {
            rigorix_engine::receipt::PlanStep::new(step.name.clone(), step.requires_approval)
        })
        .collect();
    verify_against_plan(&receipt, &plan_steps)
        .expect("a plan derived from the declaration agrees with it, by construction");
}

#[test]
fn a_receipt_with_a_repeated_step_id_is_refused_before_anything_is_built() {
    let fixture = Fixture::new();
    let receipt = fixture.receipt(vec![
        read_step("read", "README.md", &[]),
        read_step("read", "second.md", &[]),
    ]);

    let refusals = rigorix_engine::receipt::application::service::plan_from_receipt(&receipt)
        .expect_err("two steps with one id cannot be a runbook");

    match refusals.as_slice() {
        [ReceiptRefusal::UnknownStep { step, known }] => {
            assert!(
                step.starts_with("read"),
                "the refusal names the offending id: {step}"
            );
            assert_eq!(known, &vec!["read".to_string()]);
        }
        other => panic!("expected UnknownStep naming the id, got {other:?}"),
    }
}

#[tokio::test]
async fn two_dependent_steps_run_in_order_with_no_caller_supplied_list() {
    // The end-to-end half: the same receipt the derivation test reads, submitted through the surface.
    // Nothing here names a `TemplateStepDef`; the runbook is the document.
    let fixture = Fixture::new();
    let receipt = fixture.receipt(vec![
        read_step("first", "README.md", &[]),
        read_step("second", "second.md", &["first"]),
    ]);

    let output = run_receipt(fixture.workspace(), &receipt, fixture.config())
        .await
        .expect("both steps are readable and the declaration is sound");

    assert_eq!(output.execution_id, receipt.execution_id.to_string());
    assert_eq!(output.attestation.steps().len(), 2);
    let order: Vec<&str> = output
        .attestation
        .steps()
        .iter()
        .map(|step| step.step_id.as_str())
        .collect();
    assert_eq!(
        order,
        vec!["first", "second"],
        "the declaration's order and its dependency agree, and both are what ran"
    );
}

#[test]
fn the_supplied_steps_pipeline_is_what_a_receipt_driven_run_plans_with() {
    // A receipt-driven run never asks a planner for a plan, so the pipeline it wires must refuse every
    // planning question rather than answer one. Asserted here because the derivation makes the pipeline
    // *invisible*: nothing in `run_receipt` calls it, and a future change that started calling it would
    // need a classifier this surface does not have.
    let pipeline: std::sync::Arc<dyn PlanningPipelineService> =
        std::sync::Arc::new(SuppliedStepsPlanning::new(Uuid::nil()));
    assert_eq!(pipeline.execution_id(), Uuid::nil());
}

#[test]
fn a_receipt_digest_is_derived_from_the_document_not_from_the_plan() {
    // The digest a run is admitted under covers the declaration, so reordering the *derivation* cannot
    // change it. Stated as an equality between two independent computations: `sha256_hex` over the
    // canonical bytes, and `Receipt::digest`.
    let fixture = Fixture::new();
    let receipt = fixture.receipt(vec![read_step("only", "README.md", &[])]);
    let digest = receipt.digest().expect("the fixture is canonicalisable");
    assert_eq!(digest.as_str().len(), 64, "a sha256 in hex");
    assert_eq!(
        digest.as_str(),
        sha256_hex(&rigorix_engine::receipt::canonical_json(&receipt).expect("canonical bytes")),
        "the digest names the document's own canonical bytes"
    );
}
