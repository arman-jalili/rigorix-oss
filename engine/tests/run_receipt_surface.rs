#![cfg(not(feature = "portable"))]
//! `run_receipt` from **outside** the crate: the surface a host that links `rigorix-engine` uses
//! (#1619, ADR-024 §5/§10).
//!
//! This file is an integration test on purpose. It sits outside `src/`, so it can only reach what the
//! crate publishes — the same door a consumer of the tarball knocks on. A test inside the module could
//! call an internal constructor and prove nothing about the surface the issue is about: the harness
//! writes R0 by hand, verifies it, and needs a call to make.
//!
//! Two cases, and the second is why the first is not vacuous:
//!
//! 1. a receipt over **this** workspace, submitted through `run_receipt`, produces an `Attestation`
//!    whose `receipt_digest` is the receipt's own digest;
//! 2. a receipt over a **different** tree is refused by that same entry point with the typed variant
//!    (`WorkspaceMismatch`), asserted as a variant rather than as a message.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use rigorix_engine::permission::domain::PermissionMode;
use rigorix_engine::receipt::{
    BudgetDeclared, DeclaredEffect, Expectation, Receipt, ReceiptRefusal, ReceiptStep,
    RunReceiptConfig, RunReceiptError, Sandbox, SandboxRegime, Workspace, WorkspaceKind,
    run_receipt,
};

/// A temporary git repository with one commit, and a state directory **beside** it.
///
/// The engine computes the workspace identity itself (`git rev-parse HEAD^{tree}`), so a fixture that
/// skipped the repository would only ever exercise `WorkspaceIdentityUnavailable`. The state directory
/// is a sibling for the same reason the seal refuses to live inside the workspace (ADR-024 §6).
struct Fixture {
    root: PathBuf,
    state_dir: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let base = std::env::temp_dir().join(format!(
            "rigorix-run-receipt-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::SeqCst)
        ));
        let root = base.join("workspace");
        let state_dir = base.join("state");
        std::fs::create_dir_all(&root).expect("workspace dir");
        std::fs::create_dir_all(&state_dir).expect("state dir");
        std::fs::write(root.join("README.md"), "# fixture\n").expect("fixture file");
        let git = |args: &[&str]| {
            // #1083: test-only. The subject is the repository the engine will name, so a fabricated
            // tree OID would test `verify` against a value the engine never computed.
            #[allow(clippy::disallowed_types)]
            let out = std::process::Command::new("git")
                .args(args)
                .current_dir(&root)
                .output()
                .expect("git runs");
            assert!(
                out.status.success(),
                "git {args:?} failed: {}",
                String::from_utf8_lossy(&out.stderr)
            );
        };
        git(&["init", "-q"]);
        git(&["add", "README.md"]);
        git(&[
            "-c",
            "user.email=test@example.com",
            "-c",
            "user.name=test",
            "commit",
            "-q",
            "-m",
            "fixture",
        ]);
        Self { root, state_dir }
    }

    fn root(&self) -> &Path {
        &self.root
    }

    fn state_dir(&self) -> &Path {
        &self.state_dir
    }

    /// The tree OID the engine will compute, read the same way it does.
    fn tree(&self) -> String {
        #[allow(clippy::disallowed_types)]
        let out = std::process::Command::new("git")
            .args(["rev-parse", "HEAD^{tree}"])
            .current_dir(&self.root)
            .output()
            .expect("git runs");
        assert!(out.status.success(), "rev-parse failed");
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        // Guardian blocks `rm -rf`; delete the contents and then the directories.
        if let Some(base) = self.root.parent() {
            let _ = std::fs::remove_dir_all(base);
        }
    }
}

/// The fixture file's bytes, as both the runbook and the receipt see them.
fn fixture_file_contents() -> String {
    "# fixture\n".to_string()
}

/// A receipt over `tree`, declaring that its one step may read `README.md`.
fn receipt_for(tree: &str, execution_id: uuid::Uuid) -> Receipt {
    Receipt::new(
        execution_id,
        Workspace {
            kind: WorkspaceKind::CommitTree,
            value: tree.to_string(),
        },
        Sandbox {
            regime: SandboxRegime::Container,
            image_digest: "sha256:1619".to_string(),
        },
        vec![ReceiptStep {
            id: "read".to_string(),
            tool: "file-read".to_string(),
            parameters: serde_json::json!({ "path": "README.md" }),
            depends_on: Vec::new(),
            declared_effects: DeclaredEffect {
                paths: vec!["README.md".to_string()],
                // One digest per declared path, derived from the fixture's own bytes: a declaration
                // with a path and no digest is refused before the graph, on the rule that a path
                // nobody can check is not a declared effect.
                content_digests: vec![rigorix_engine::receipt::sha256_hex(
                    fixture_file_contents().as_bytes(),
                )],
            },
            expectation: Expectation {
                description: "reads the README".to_string(),
                quality_level: None,
            },
            requires_approval: false,
            timeout_secs: None,
        }],
        BudgetDeclared {
            max_iterations: 4,
            max_tokens: 100,
        },
        rigorix_engine::quality_gates::domain::level::QualityLevel::MergeReady,
    )
    .expect("the fixture carries no credentials")
}

fn config(fixture: &Fixture) -> RunReceiptConfig {
    RunReceiptConfig {
        state_dir: fixture.state_dir().to_path_buf(),
        audit_key: Some("run-receipt-test-key".to_string()),
        permission_mode: PermissionMode::WorkspaceWrite,
    }
}

/// The acceptance: a consumer builds the receipt, submits it, and reads back the observation whose
/// `receipt_digest` is the receipt's own.
#[tokio::test]
async fn a_submitted_receipt_returns_an_attestation_that_answers_it() {
    let fixture = Fixture::new();
    let execution_id = uuid::Uuid::now_v7();
    let receipt = receipt_for(&fixture.tree(), execution_id);

    let output = run_receipt(fixture.root(), &receipt, config(&fixture))
        .await
        .expect("a receipt over this workspace must be accepted");

    assert_eq!(
        output.receipt_digest,
        receipt
            .digest()
            .expect("the fixture receipt has a digest")
            .to_string(),
        "the run reports the declaration it was admitted under"
    );
    // The point of the whole surface: the *observation* names the declaration. Equality, not presence:
    // an attestation that carries some digest is not the same claim as one that carries this one.
    assert_eq!(
        output.attestation.receipt_digest(),
        output.receipt_digest,
        "the attestation answers the receipt the run was admitted under"
    );
    assert_eq!(output.attestation.execution_id(), execution_id);
    assert_eq!(output.execution_id, execution_id.to_string());
    assert!(
        !output.attestation_digest.is_empty(),
        "a sealed attestation has a digest the envelope can reference"
    );
}

/// A receipt that names another tree is refused, and the refusal is the variant — asserted as a
/// variant, because a refusal is a typed answer and a message is prose.
#[tokio::test]
async fn a_receipt_for_another_tree_is_refused_by_the_same_entry_point() {
    let fixture = Fixture::new();
    let receipt = receipt_for(
        "0000000000000000000000000000000000000000",
        uuid::Uuid::now_v7(),
    );

    let error = run_receipt(fixture.root(), &receipt, config(&fixture))
        .await
        .expect_err("a receipt naming another tree must be refused");

    match error {
        RunReceiptError::Refused(refusals) => {
            assert!(
                refusals
                    .iter()
                    .any(|r| matches!(r, ReceiptRefusal::WorkspaceMismatch { .. })),
                "the refusal is the workspace rule, not a neighbour: {refusals:?}"
            );
        }
        other => panic!("a refusal must be typed, not infrastructural: {other:?}"),
    }
}

/// An unsigned run cannot produce an observation, and says so instead of writing an unsealed record.
#[tokio::test]
async fn a_run_without_a_key_refuses_rather_than_writing_an_unsigned_attestation() {
    let fixture = Fixture::new();
    let receipt = receipt_for(&fixture.tree(), uuid::Uuid::now_v7());
    let mut cfg = config(&fixture);
    cfg.audit_key = None;

    let error = run_receipt(fixture.root(), &receipt, cfg)
        .await
        .expect_err("an unsigned run has no attestation to return");

    // The store's own refusal, carried as infrastructure: "could not be recorded", not "the receipt
    // was bad". The declaration was fine; the deployment had no key.
    match error {
        RunReceiptError::Infrastructure { detail } => assert!(
            detail.contains("attestation") || detail.contains("sign"),
            "the reason names the seal: {detail}"
        ),
        other => panic!("a missing key is not a refusal of the receipt: {other:?}"),
    }
}
