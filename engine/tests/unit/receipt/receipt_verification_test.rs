//! The receipt is a whole-value check before anything runs (#1527).
//!
//! Each test builds its own fixture, so the expectation is owned here rather than read from the
//! implementation: a table-driven test whose cases come from the code under test proves only that
//! the code agrees with itself.
//!
//! Gate: `receipt` is engine-only (it reads `quality_gates` and `uuid`), so this module carries the
//! same `#[cfg(not(feature = "portable"))]` gate as its siblings.

use rigorix_engine::quality_gates::domain::level::QualityLevel;
use rigorix_engine::receipt::domain::attestation::ObservedEffect;
use rigorix_engine::receipt::{
    AttestationBuilder, BudgetDeclared, DeclaredEffect, Expectation, QualitySource, Receipt,
    ReceiptRefusal, ReceiptStep, ReceiptVerdict, Role, Sandbox, SandboxRegime, Workspace,
    WorkspaceKind, compare_declared_observed, verify,
};
use uuid::Uuid;

fn workspace() -> Workspace {
    Workspace {
        kind: WorkspaceKind::CommitTree,
        value: "4b825dc642cb6eb9a060e54bf8d69288fbee4904".to_string(),
    }
}

fn sandbox() -> Sandbox {
    Sandbox {
        regime: SandboxRegime::Container,
        image_digest: "sha256:1111".to_string(),
    }
}

fn step(id: &str, depends_on: &[&str]) -> ReceiptStep {
    ReceiptStep {
        id: id.to_string(),
        tool: "file_write".to_string(),
        parameters: serde_json::json!({ "path": format!("{id}.txt") }),
        depends_on: depends_on.iter().map(|d| (*d).to_string()).collect(),
        declared_effects: DeclaredEffect {
            paths: vec![format!("{id}.txt")],
            content_digests: vec![format!("digest-of-{id}")],
        },
        expectation: Expectation {
            description: format!("{id} writes its file"),
            quality_level: None,
        },
        requires_approval: false,
        timeout_secs: None,
    }
}

fn receipt(steps: Vec<ReceiptStep>) -> Receipt {
    Receipt::new(
        Uuid::nil(),
        workspace(),
        sandbox(),
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
        QualityLevel::MergeReady,
    )
    .expect("the fixture carries no credentials")
}

#[test]
fn receipt_verification_a_sound_receipt_is_verified() {
    let receipt = receipt(vec![step("a", &[]), step("b", &["a"])]);
    let verified = verify(&receipt, &workspace().value).expect("verifies");
    assert_eq!(verified.receipt().steps.len(), 2);
    assert_eq!(verified.digest().len(), 64, "a sha256 in hex");
}

#[test]
fn receipt_verification_schema_version_names_the_supported_set() {
    let mut receipt = receipt(vec![step("a", &[])]);
    receipt.schema_version = 99;
    let refusals = verify(&receipt, &workspace().value).unwrap_err();
    match refusals.as_slice() {
        [ReceiptRefusal::SchemaVersion { found, supported }] => {
            assert_eq!(*found, 99);
            assert_eq!(supported, &vec![1]);
        }
        other => panic!("expected one SchemaVersion refusal, got {other:?}"),
    }
}

#[test]
fn receipt_verification_workspace_mismatch_names_both_identities() {
    let receipt = receipt(vec![step("a", &[])]);
    let refusals = verify(&receipt, "0000000000000000000000000000000000000000").unwrap_err();
    match refusals.as_slice() {
        [
            ReceiptRefusal::WorkspaceMismatch {
                receipt: named,
                workspace: given,
            },
        ] => {
            assert_eq!(named, &workspace().value);
            assert_eq!(given, "0000000000000000000000000000000000000000");
        }
        other => panic!("expected WorkspaceMismatch, got {other:?}"),
    }
    // …and the same receipt against the workspace the engine computed verifies. The comparison is
    // the engine's answer either way, never the receipt's own string.
    assert!(verify(&receipt, &workspace().value).is_ok());
}

#[test]
fn receipt_verification_an_unknown_dependency_names_the_step_and_the_known_ones() {
    let receipt = receipt(vec![step("a", &["ghost"])]);
    let refusals = verify(&receipt, &workspace().value).unwrap_err();
    assert!(
        refusals.iter().any(|r| matches!(
            r,
            ReceiptRefusal::UnknownStep { step, known } if step == "ghost" && known == &vec!["a".to_string()]
        )),
        "{refusals:?}"
    );
}

#[test]
fn receipt_verification_a_cycle_names_every_step_it_holds() {
    let receipt = receipt(vec![step("a", &["b"]), step("b", &["a"])]);
    let refusals = verify(&receipt, &workspace().value).unwrap_err();
    let cycle = refusals
        .iter()
        .find_map(|r| match r {
            ReceiptRefusal::DependencyCycle { steps } => Some(steps.clone()),
            _ => None,
        })
        .expect("a cycle refusal");
    let mut cycle = cycle;
    cycle.sort();
    assert_eq!(cycle, vec!["a".to_string(), "b".to_string()]);
}

#[test]
fn receipt_verification_an_empty_sandbox_image_is_sandbox_missing() {
    let mut receipt = receipt(vec![step("a", &[])]);
    receipt.sandbox.image_digest = "  ".to_string();
    let refusals = verify(&receipt, &workspace().value).unwrap_err();
    assert!(
        refusals.contains(&ReceiptRefusal::SandboxMissing),
        "{refusals:?}"
    );
}

#[test]
fn receipt_verification_a_credential_is_refused_at_construction_and_not_quoted() {
    let leaky = ReceiptStep {
        parameters: serde_json::json!({ "options": { "api_key": "sk-live-do-not-log" } }),
        ..step("a", &[])
    };
    let err = Receipt::new(
        Uuid::nil(),
        workspace(),
        sandbox(),
        vec![leaky],
        vec![],
        BudgetDeclared {
            max_iterations: 1,
            max_tokens: 1,
        },
        QualityLevel::TargetedTests,
    )
    .unwrap_err();
    match err {
        ReceiptRefusal::SecretInParameters { step, path } => {
            assert_eq!(step, "a");
            assert_eq!(path, "options.api_key");
        }
        other => panic!("expected SecretInParameters, got {other:?}"),
    }
    // A refusal that quotes the secret puts it in the log the refusal exists to keep it out of.
    let message = ReceiptRefusal::SecretInParameters {
        step: "a".to_string(),
        path: "options.api_key".to_string(),
    }
    .to_string();
    assert!(!message.contains("sk-live-do-not-log"), "{message}");
}

#[test]
fn receipt_verification_a_sealed_digest_that_no_longer_matches_is_refused() {
    let mut receipt = receipt(vec![step("a", &[])]);
    receipt.set_digest = Some("not-the-digest".to_string());
    let refusals = verify(&receipt, &workspace().value).unwrap_err();
    assert!(
        refusals
            .iter()
            .any(|r| matches!(r, ReceiptRefusal::DigestMismatch { .. })),
        "{refusals:?}"
    );
}

#[test]
fn receipt_verification_the_canonical_form_ignores_member_order_and_not_content() {
    // Two receipts that differ only in JSON member order have one digest; one changed declared path
    // has another. The canonical form is the reason, so this is asserted on the value, not on a
    // hand-written string.
    let first = receipt(vec![step("a", &[])]);
    let mut reordered = first.clone();
    reordered.roles = vec![Role {
        structured_output: false,
        context_window: first.roles[0].context_window,
        model_id: first.roles[0].model_id.clone(),
        role: first.roles[0].role.clone(),
    }];
    assert_eq!(first.digest().unwrap(), reordered.digest().unwrap());

    let mut changed = first.clone();
    changed.steps[0].declared_effects.paths[0] = "elsewhere.txt".to_string();
    assert_ne!(first.digest().unwrap(), changed.digest().unwrap());
}

#[test]
fn receipt_verification_an_undeclared_write_is_refused_with_the_path_named() {
    let declared = DeclaredEffect {
        paths: vec!["a.txt".to_string()],
        content_digests: vec!["digest-of-a".to_string()],
    };
    let refusals = compare_declared_observed(
        "a",
        &declared,
        &[ObservedEffect {
            path: "elsewhere.txt".to_string(),
            content_digest: "whatever".to_string(),
        }],
    );
    match refusals.as_slice() {
        [
            ReceiptRefusal::UndeclaredWrite {
                step,
                path,
                declared: list,
            },
        ] => {
            assert_eq!(step, "a");
            assert_eq!(path, "elsewhere.txt");
            assert_eq!(list, &vec!["a.txt".to_string()]);
        }
        other => panic!("expected UndeclaredWrite, got {other:?}"),
    }
}

#[test]
fn receipt_verification_a_content_digest_that_differs_is_refused() {
    let declared = DeclaredEffect {
        paths: vec!["a.txt".to_string()],
        content_digests: vec!["declared".to_string()],
    };
    let refusals = compare_declared_observed(
        "a",
        &declared,
        &[ObservedEffect {
            path: "a.txt".to_string(),
            content_digest: "observed".to_string(),
        }],
    );
    match refusals.as_slice() {
        [
            ReceiptRefusal::ContentDigestMismatch {
                declared: expected,
                observed,
                ..
            },
        ] => {
            assert_eq!(expected, "declared");
            assert_eq!(observed, "observed");
        }
        other => panic!("expected ContentDigestMismatch, got {other:?}"),
    }
}

#[test]
fn receipt_verification_a_declared_path_without_its_digest_is_refused() {
    // A paths/digests pair that is not a pair cannot be compared later, and a comparison that
    // truncated to the shorter list is how an undeclared write passes.
    let mut broken = step("a", &[]);
    broken.declared_effects.content_digests.clear();
    let receipt = receipt(vec![broken]);
    let refusals = verify(&receipt, &workspace().value).unwrap_err();
    assert!(
        refusals
            .iter()
            .any(|r| matches!(r, ReceiptRefusal::ContentDigestMismatch { .. })),
        "{refusals:?}"
    );
}

#[test]
fn receipt_verification_a_refused_receipt_dispatches_nothing() {
    // The gate is the type: `verify` is the only producer of a `VerifiedReceipt`, and the builder
    // starts from one. So a refused receipt has no path to a builder, and the counter the test
    // passes over the Ok branch is never reached.
    let refused = {
        let mut receipt = receipt(vec![step("a", &["ghost"])]);
        receipt.set_digest = Some("wrong".to_string());
        receipt
    };
    let mut dispatched = 0usize;
    match verify(&refused, &workspace().value) {
        Ok(verified) => {
            let _ = AttestationBuilder::from_verified(&verified);
            dispatched += 1;
        }
        Err(refusals) => {
            assert!(!refusals.is_empty(), "a refusal list is never empty");
            // Nothing was built, so nothing could be dispatched.
            assert_eq!(dispatched, 0);
        }
    }
    assert_eq!(dispatched, 0);

    // And the verdict shape is the same judgement in the shape `GateOutcome` uses.
    let verdict = refused.verdict_against(&workspace().value);
    assert!(matches!(verdict, ReceiptVerdict::Refused(_)));
    assert!(!verdict.is_verified());
    assert!(!verdict.refusals().is_empty());
}

#[test]
fn receipt_verification_the_budget_is_echoed_and_says_it_was_not_enforced() {
    let receipt = receipt(vec![step("a", &[])]);
    let verified = verify(&receipt, &workspace().value).expect("verifies");
    let builder = AttestationBuilder::from_verified(&verified);
    let attestation = builder.build(2, 42);
    let budget = attestation.budget();
    assert_eq!(budget.declared_iterations, 10);
    assert_eq!(budget.observed_iterations, 2);
    assert_eq!(budget.observed_tokens, 42);
    assert!(
        !budget.enforced,
        "this change adds no pre-dispatch budget check"
    );
}

#[test]
fn receipt_verification_a_caller_claim_is_recorded_as_a_claim() {
    // The caller declared `merge-ready`; no step executed it. The record must say so, and there is no
    // field that reads as "passed".
    let receipt = receipt(vec![step("a", &[])]);
    let verified = verify(&receipt, &workspace().value).expect("verifies");
    let builder = AttestationBuilder::from_verified(&verified);
    let attestation = builder.build(1, 1);
    assert_eq!(attestation.quality().level, QualityLevel::MergeReady);
    assert_eq!(attestation.quality().source, QualitySource::Caller);
}

#[test]
fn receipt_verification_a_level_a_step_established_is_sourced_to_the_engine() {
    // The caller asked for targeted tests; a step established the workspace level, which is more
    // than was asked for. That is the one case where the record is the engine's.
    let mut receipt = receipt(vec![step("a", &[])]);
    receipt.quality_required.level = QualityLevel::TargetedTests;
    let verified = verify(&receipt, &workspace().value).expect("verifies");
    let mut builder = AttestationBuilder::from_verified(&verified);
    builder.quality_from_step("a", QualityLevel::Workspace);
    let attestation = builder.build(1, 1);
    assert_eq!(attestation.quality().level, QualityLevel::Workspace);
    match &attestation.quality().source {
        QualitySource::Engine { step_id } => assert_eq!(step_id, "a"),
        other => panic!("expected an engine-sourced level, got {other:?}"),
    }
}

#[test]
fn receipt_verification_a_lower_step_level_does_not_overwrite_a_caller_claim() {
    // A claim above what the engine observed stays the caller's claim; a claim below what a step
    // established does not lower the record. Either way the source is the honest one.
    let receipt = receipt(vec![step("a", &[])]);
    let verified = verify(&receipt, &workspace().value).expect("verifies");
    let mut builder = AttestationBuilder::from_verified(&verified);
    builder.quality_from_step("a", QualityLevel::TargetedTests);
    let attestation = builder.build(1, 1);
    assert_eq!(attestation.quality().level, QualityLevel::MergeReady);
    assert_eq!(attestation.quality().source, QualitySource::Caller);
}
#[test]
fn receipt_verification_a_state_dir_inside_the_workspace_is_refused_naming_both_paths() {
    // ADR-024 §6 rule 1. The subject is the resolved location, not the string: the second half of
    // this test asks for the inside path through `x/..` and must still be refused, because a rule
    // that a spelling can defeat is a rule about spelling.
    let repo = tempdir("w");
    let inside = repo.path().join(".rigorix").join("state");
    let err = rigorix_engine::receipt::refuse_state_dir_inside_workspace(&inside, repo.path())
        .expect_err("inside the workspace must be refused");
    match err {
        ReceiptRefusal::StateDirInsideWorkspace {
            state_dir,
            repo_root,
        } => {
            assert!(
                state_dir.ends_with(".rigorix/state") || state_dir.ends_with(".rigorix\\state")
            );
            assert_eq!(
                std::path::Path::new(&repo_root).canonicalize().ok(),
                repo.path().canonicalize().ok()
            );
        }
        other => panic!("expected StateDirInsideWorkspace, got {other:?}"),
    }

    let disguised = repo
        .path()
        .join("sub")
        .join("..")
        .join(".rigorix")
        .join("state");
    assert!(
        rigorix_engine::receipt::refuse_state_dir_inside_workspace(&disguised, repo.path())
            .is_err(),
        "a `..` spelling does not move the store outside the workspace"
    );

    let outside = repo.path().parent().unwrap().join("elsewhere-state");
    assert!(
        rigorix_engine::receipt::refuse_state_dir_inside_workspace(&outside, repo.path()).is_ok(),
        "a sibling is outside the workspace"
    );
}

#[test]
fn receipt_verification_the_default_state_dir_is_a_sibling_and_the_variable_overrides_it() {
    // The default has to satisfy the refusal above without configuration, or every host is broken on
    // the day this lands.
    let repo = tempdir("d");
    let root = repo.path().to_string_lossy().to_string();
    let default = rigorix_engine::receipt::default_state_dir(&root);
    assert!(
        rigorix_engine::receipt::refuse_state_dir_inside_workspace(&default, repo.path()).is_ok(),
        "the default must be outside the workspace: {}",
        default.display()
    );
    assert!(default.to_string_lossy().contains(".execution-state"));

    // `RIGORIX_STATE_DIR` wins when set. Set and removed inside one test, which is why this reads
    // the value back rather than asserting on a second process.
    let declared = repo.path().parent().unwrap().join("declared-state");
    // SAFETY-adjacent: single test in this file touches the variable, and it is restored below.
    unsafe { std::env::set_var("RIGORIX_STATE_DIR", &declared) };
    let overridden = rigorix_engine::receipt::default_state_dir(&root);
    unsafe { std::env::remove_var("RIGORIX_STATE_DIR") };
    assert_eq!(overridden, declared);
}

/// A temporary directory that the test owns and that is removed with it.
struct TempDir(std::path::PathBuf);

impl TempDir {
    fn path(&self) -> &std::path::Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn tempdir(tag: &str) -> TempDir {
    let base = std::env::temp_dir().join(format!(
        "rigorix-receipt-{tag}-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    std::fs::create_dir_all(&base).expect("a temporary directory");
    TempDir(base)
}
