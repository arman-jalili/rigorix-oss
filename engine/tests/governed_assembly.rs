#![cfg(not(feature = "portable"))]
//! One governed assembly: `run_receipt` consults the consequence gate, because it no longer builds
//! its own engine (#1625, ADR-025 D1/D5).
//!
//! Before this, `receipt::api::run_receipt` wired four collaborators by hand — audit key, permission
//! mode, state dir, sandbox — and attached no precondition gate, no sequence policy, no capacity
//! ledger, no effect commitment and no approval store. The only way to notice was to read the file:
//! a run under a denying `.rigorix/preconditions.toml` dispatched, and its output was identical to a
//! run whose gate found nothing. "No findings" and "no gate" are the same output.
//!
//! These tests assert the difference from **outside** the crate, through the published surface, so
//! they prove the property a host gets rather than the wiring this repository happens to have.
//!
//! Gate: the receipt module is engine-only (it reads `quality_gates`, outside the portable slice).

use std::path::{Path, PathBuf};

use rigorix_engine::permission::domain::PermissionMode;
use rigorix_engine::quality_gates::domain::level::QualityLevel;
use rigorix_engine::receipt::{
    BudgetDeclared, DeclaredEffect, Expectation, Receipt, ReceiptStep, RunReceiptConfig,
    RunReceiptError, Sandbox, SandboxRegime, Workspace, WorkspaceKind, run_receipt,
};
use uuid::Uuid;

const DENIED_STEP: &str = "read";
const GATE_ID: &str = "readability-guard";

/// A repository with one commit, a sibling state directory, and whatever `.rigorix/` the test writes.
struct Fixture {
    root: tempfile::TempDir,
    tree: String,
    state: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let root = tempfile::tempdir().expect("temp root");
        let path = root.path();
        // A sink by the repository's own taxonomy — a test fixture shelling out to `git` to build the
        // tree a receipt names. It is not a production crossing, and the alternative (a hand-written
        // tree hash) would be a fixture asserting its own arithmetic.
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
        run(&["add", "."]);
        run(&["commit", "-q", "-m", "fixture"]);
        let tree = run(&["rev-parse", "HEAD^{tree}"]);
        // The state directory is a sibling of the repository, as ADR-024 §6 requires: a seal inside
        // the workspace is one the run could rewrite.
        let state = path
            .parent()
            .expect("the fixture has a parent")
            .join(format!("state-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&state).expect("state directory");
        Self { root, tree, state }
    }

    fn workspace(&self) -> &Path {
        self.root.path()
    }

    fn rigorix(&self) -> PathBuf {
        self.root.path().join(".rigorix")
    }

    /// Arm the dispatch-time gate: `tool = "file-read"` steps are refused by a real process check
    /// (`/bin/sh -c 'exit 1'`), which is a genuine non-zero exit rather than a mock.
    fn deny_file_reads(&self) {
        std::fs::create_dir_all(self.rigorix()).expect(".rigorix");
        std::fs::write(
            self.rigorix().join("preconditions.toml"),
            format!(
                r#"
[[preconditions]]
id = "{GATE_ID}"
match = {{ tool = "file-read" }}
command = ["/bin/sh", "-c", "exit 1"]

[gating]
release_dependents_on_failure = false
"#
            ),
        )
        .expect("preconditions");
    }

    /// The same file, with a check that passes: the control that proves the gate is *consulted*
    /// rather than refusing everything it sees.
    fn allow_file_reads(&self) {
        std::fs::create_dir_all(self.rigorix()).expect(".rigorix");
        std::fs::write(
            self.rigorix().join("preconditions.toml"),
            format!(
                r#"
[[preconditions]]
id = "{GATE_ID}"
match = {{ tool = "file-read" }}
command = ["/bin/sh", "-c", "exit 0"]

[gating]
release_dependents_on_failure = false
"#
            ),
        )
        .expect("preconditions");
    }

    /// A file that cannot be parsed. ADR-017's rule is fail-closed: governance that cannot be
    /// established must not silently vanish, so the assembly arms a gate that refuses everything.
    fn malformed_preconditions(&self) {
        std::fs::create_dir_all(self.rigorix()).expect(".rigorix");
        std::fs::write(
            self.rigorix().join("preconditions.toml"),
            "[[preconditions]\nid = this is not toml\n",
        )
        .expect("preconditions");
    }

    fn receipt(&self, execution_id: Uuid) -> Receipt {
        Receipt::new(
            execution_id,
            Workspace {
                kind: WorkspaceKind::CommitTree,
                value: self.tree.clone(),
            },
            Sandbox {
                regime: SandboxRegime::Container,
                image_digest: "sha256:fixture".to_string(),
            },
            vec![ReceiptStep {
                id: DENIED_STEP.to_string(),
                tool: "file-read".to_string(),
                parameters: serde_json::json!({ "path": "README.md" }),
                depends_on: Vec::new(),
                // This step writes nothing: it reads. An empty declaration is a statement, not an
                // absence — the run's observed effects are compared against it.
                declared_effects: DeclaredEffect {
                    paths: Vec::new(),
                    content_digests: Vec::new(),
                },
                expectation: Expectation {
                    description: "the fixture file is read".to_string(),
                    quality_level: None,
                },
                requires_approval: false,
                timeout_secs: Some(10),
            }],
            BudgetDeclared {
                max_iterations: 1,
                max_tokens: 1000,
            },
            QualityLevel::MergeReady,
        )
        .expect("the fixture receipt carries no credentials")
    }

    fn config(&self) -> RunReceiptConfig {
        RunReceiptConfig {
            state_dir: self.state.clone(),
            audit_key: Some("assembly-fixture-key".to_string()),
            permission_mode: PermissionMode::WorkspaceWrite,
        }
    }
}

fn refusal_code(error: &RunReceiptError) -> String {
    match error {
        RunReceiptError::RefusedAtGate { code, .. } => code.clone(),
        other => panic!("expected a gate refusal, got {other:?}"),
    }
}

#[tokio::test]
async fn a_denying_precondition_refuses_the_run_through_the_receipt_surface() {
    let fixture = Fixture::new();
    fixture.deny_file_reads();
    let execution_id = Uuid::now_v7();

    let error = run_receipt(
        fixture.workspace(),
        &fixture.receipt(execution_id),
        fixture.config(),
    )
    .await
    .expect_err("the operator's gate denies this step, so the run must not produce an attestation");

    assert_eq!(
        refusal_code(&error),
        "precondition_denied",
        "the dispatcher's code, carried rather than re-spelled"
    );
    match &error {
        RunReceiptError::RefusedAtGate { step, .. } => assert_eq!(step, DENIED_STEP),
        other => panic!("expected RefusedAtGate, got {other:?}"),
    }
}

#[tokio::test]
async fn a_passing_precondition_lets_the_run_through() {
    // The control. A gate that refuses everything is not a gate, and a test that only ever sees a
    // refusal cannot tell "consulted and denied" from "consulted and allowed" — or from "never built".
    let fixture = Fixture::new();
    fixture.allow_file_reads();
    let execution_id = Uuid::now_v7();

    let output = run_receipt(
        fixture.workspace(),
        &fixture.receipt(execution_id),
        fixture.config(),
    )
    .await
    .expect("the operator's gate allows this step");

    assert_eq!(output.execution_id, execution_id.to_string());
}

#[tokio::test]
async fn a_malformed_precondition_file_refuses_the_run_rather_than_disappearing() {
    // ADR-017's fail-closed rule, reached through every host rather than through `GateSetup` alone:
    // a file that cannot be parsed arms an unarmed gate, so the failure is a refusal to run — not a
    // dispatch with no governance and a log line nobody reads.
    let fixture = Fixture::new();
    fixture.malformed_preconditions();
    let execution_id = Uuid::now_v7();

    let error = run_receipt(
        fixture.workspace(),
        &fixture.receipt(execution_id),
        fixture.config(),
    )
    .await
    .expect_err("governance that cannot be established must not vanish");

    assert_eq!(refusal_code(&error), "precondition_denied");
}

#[tokio::test]
async fn the_run_refusal_is_not_a_refusal_of_the_declaration() {
    // The three states a caller must be able to tell apart: the declaration was refused (nothing
    // dispatched), the gate refused a step (admitted, not performed), the engine could not record
    // what happened. Collapsing them into one variant would make "your receipt is wrong" and "your
    // operator said no" the same answer.
    let fixture = Fixture::new();
    fixture.deny_file_reads();
    let mut receipt = fixture.receipt(Uuid::now_v7());
    receipt.workspace.value = "not-this-tree".to_string();

    let error = run_receipt(fixture.workspace(), &receipt, fixture.config())
        .await
        .expect_err("the declaration names another tree");

    match error {
        RunReceiptError::Refused(refusals) => assert!(
            refusals.iter().any(|refusal| matches!(
                refusal,
                rigorix_engine::receipt::ReceiptRefusal::WorkspaceMismatch { .. }
            )),
            "a foreign workspace is a refusal of the declaration, not a gate refusal: {refusals:?}"
        ),
        other => panic!("expected the declaration to be refused, got {other:?}"),
    }
}
