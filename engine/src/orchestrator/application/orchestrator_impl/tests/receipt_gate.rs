//! The receipt is verified before the graph exists (#1527, ADR-024 §4).
//!
//! `verify` runs before the identity gate, before the policy pipeline, before any state is written
//! and — the claim these cases exist for — before `build_graph`. "The receipt was refused" and
//! "nothing ran" are then the same statement rather than two claims a reader has to correlate.
//!
//! # Why the assertion is an absence, and why it is not vacuous
//!
//! An assertion on the refusal's message would pass with the verification moved *after* the graph
//! was built: the same error, the same words, a graph already allocated. So the refusal case asserts
//! what did **not** happen (the executor was never asked to run a graph, so no record and no envelope
//! exist either), and it is paired with a control that runs: a verified receipt reaches the executor
//! exactly once and its digest reaches the envelope. Without that control the absence case would pass
//! on a harness that never dispatches anything for any reason.
//!
//! The mutation that proves the ordering is recorded in the pull request: swapping the two calls in
//! `run_from_template` turns the refusal case red (the counter reaches 1).

use std::sync::atomic::{AtomicUsize, Ordering};

use super::*;
use crate::receipt::{
    BudgetDeclared, DeclaredEffect, Expectation, Receipt, ReceiptStep, Role, Sandbox,
    SandboxRegime, Workspace, WorkspaceKind,
};

// ---------------------------------------------------------------------------
// A repository whose tree the engine can name
// ---------------------------------------------------------------------------

/// A temporary git repository with one empty commit.
///
/// The engine computes the workspace identity itself (`git rev-parse HEAD^{tree}`) and a receipt is
/// compared against *that*, so a test that skipped the repository would only ever exercise
/// `WorkspaceIdentityUnavailable`. The tree is a real one and the test reads it back through the
/// same function the engine uses.
struct TempRepo {
    path: std::path::PathBuf,
}

impl TempRepo {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "rigorix-receipt-gate-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::SeqCst)
        ));
        std::fs::create_dir_all(&path).expect("temp dir");
        let git = |args: &[&str]| {
            // #1083: test-only. The subject is the repository the engine will name, so a fabricated
            // tree OID would test `verify` against a value the engine never computed.
            #[allow(clippy::disallowed_types)]
            let out = std::process::Command::new("git")
                .args(args)
                .current_dir(&path)
                .output()
                .expect("git runs");
            assert!(
                out.status.success(),
                "git {args:?} failed: {}",
                String::from_utf8_lossy(&out.stderr)
            );
        };
        git(&["init", "-q"]);
        git(&[
            "-c",
            "user.email=test@example.com",
            "-c",
            "user.name=test",
            "commit",
            "-q",
            "--allow-empty",
            "-m",
            "fixture",
        ]);
        Self { path }
    }

    fn root(&self) -> String {
        self.path.to_string_lossy().to_string()
    }
}

impl Drop for TempRepo {
    fn drop(&mut self) {
        // `remove_dir_all`, not a shell: this is the guard for every case below, and a failed
        // cleanup must not fail the test it belongs to.
        std::fs::remove_dir_all(&self.path).ok();
    }
}

// ---------------------------------------------------------------------------
// An executor that counts what it was asked to do
// ---------------------------------------------------------------------------

/// Forwards to `MockExecutionService` and counts the `execute_graph` calls.
///
/// A double for a trait the tree already has, and the third absence in this file: the executor is
/// the only publisher of `NodeStarted`, so "the executor was never called" is also "no node ever
/// started" — the assertion does not need the event bus to say it twice.
struct CountingExecutionService {
    calls: Arc<AtomicUsize>,
}

#[async_trait]
impl exec_svc::ParallelExecutionService for CountingExecutionService {
    async fn execute_graph(
        &self,
        input: exec_dto::ExecuteGraphInput,
    ) -> Result<exec_dto::ExecuteGraphOutput, crate::execution_engine::domain::ExecutionError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        crate::orchestrator::application::orchestrator_mocks::MockExecutionService
            .execute_graph(input)
            .await
    }

    async fn execute_node(
        &self,
        input: exec_dto::ExecuteNodeInput,
    ) -> Result<exec_dto::ExecuteNodeOutput, crate::execution_engine::domain::ExecutionError> {
        crate::orchestrator::application::orchestrator_mocks::MockExecutionService
            .execute_node(input)
            .await
    }

    async fn get_execution_state(
        &self,
        input: exec_dto::GetExecutionStateInput,
    ) -> Result<exec_dto::GetExecutionStateOutput, crate::execution_engine::domain::ExecutionError>
    {
        crate::orchestrator::application::orchestrator_mocks::MockExecutionService
            .get_execution_state(input)
            .await
    }

    async fn pause_execution(
        &self,
        input: exec_dto::PauseExecutionInput,
    ) -> Result<exec_dto::PauseExecutionOutput, crate::execution_engine::domain::ExecutionError>
    {
        crate::orchestrator::application::orchestrator_mocks::MockExecutionService
            .pause_execution(input)
            .await
    }

    async fn resume_execution(
        &self,
        input: exec_dto::ResumeExecutionInput,
    ) -> Result<exec_dto::ResumeExecutionOutput, crate::execution_engine::domain::ExecutionError>
    {
        crate::orchestrator::application::orchestrator_mocks::MockExecutionService
            .resume_execution(input)
            .await
    }

    async fn approve_node(
        &self,
        input: exec_dto::ApproveNodeInput,
    ) -> Result<exec_dto::ApproveNodeOutput, crate::execution_engine::domain::ExecutionError> {
        crate::orchestrator::application::orchestrator_mocks::MockExecutionService
            .approve_node(input)
            .await
    }

    async fn hydrate_execution(
        &self,
        input: exec_dto::HydrateExecutionInput,
    ) -> Result<exec_dto::HydrateExecutionOutput, crate::execution_engine::domain::ExecutionError>
    {
        crate::orchestrator::application::orchestrator_mocks::MockExecutionService
            .hydrate_execution(input)
            .await
    }

    fn on_progress(
        &self,
        callback: Box<
            dyn Fn(crate::execution_engine::application::service::ExecutionProgress) + Send + Sync,
        >,
    ) {
        crate::orchestrator::application::orchestrator_mocks::MockExecutionService
            .on_progress(callback);
    }

    async fn abort_execution(
        &self,
        input: exec_dto::AbortExecutionInput,
    ) -> Result<exec_dto::AbortExecutionOutput, crate::execution_engine::domain::ExecutionError>
    {
        crate::orchestrator::application::orchestrator_mocks::MockExecutionService
            .abort_execution(input)
            .await
    }
}

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

fn step_def() -> crate::orchestrator::dto::TemplateStepDef {
    crate::orchestrator::dto::TemplateStepDef {
        name: "write".to_string(),
        tool: "run_command".to_string(),
        description: "write one file".to_string(),
        parameters: serde_json::json!({ "command": "true" }),
        require_identity: false,
        requires_approval: false,
        timeout_secs: None,
        evaluate_score: false,
        depends_on: Vec::new(),
    }
}

/// A receipt that names `workspace` as the tree it applies to.
fn receipt_for(workspace: &str) -> Receipt {
    Receipt::new(
        Uuid::nil(),
        Workspace {
            kind: WorkspaceKind::CommitTree,
            value: workspace.to_string(),
        },
        Sandbox {
            regime: SandboxRegime::Container,
            image_digest: "sha256:1111".to_string(),
        },
        vec![ReceiptStep {
            id: "write".to_string(),
            tool: "run_command".to_string(),
            parameters: serde_json::json!({ "command": "true" }),
            depends_on: Vec::new(),
            declared_effects: DeclaredEffect {
                paths: vec!["out.txt".to_string()],
                content_digests: vec!["digest-of-out".to_string()],
            },
            expectation: Expectation {
                description: "writes one file".to_string(),
                quality_level: None,
            },
            requires_approval: false,
            timeout_secs: None,
        }],
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
        crate::quality_gates::domain::level::QualityLevel::MergeReady,
    )
    .expect("the fixture carries no credentials")
}

fn input(repo_root: String, receipt: Receipt) -> RunFromTemplateInput {
    RunFromTemplateInput {
        steps: vec![step_def()],
        repo_root,
        execution_id: Some(Uuid::new_v4()),
        template_name: "receipt-gate".to_string(),
        repository: None,
        author: Some("test@example.com".to_string()),
        identity: None,
        identity_verification_failure: None,
        enforcement_preset: None,
        receipt: Some(receipt),
    }
}

/// The harness: mocks everywhere except the executor (counted) and audit (captured).
async fn service(
    calls: Arc<AtomicUsize>,
    captured: Arc<std::sync::Mutex<Option<audit_app::BuildEnvelopeInput>>>,
    policy: Arc<super::RecordingPolicy>,
) -> OrchestratorServiceImpl {
    use crate::orchestrator::application::orchestrator_mocks as mocks;
    OrchestratorServiceImpl::new(
        crate::orchestrator::domain::OrchestratorConfig::default(),
        Arc::new(mocks::MockPlanningService::new()),
        Arc::new(CountingExecutionService { calls }),
        Arc::new(mocks::MockStateService::new()),
        Arc::new(mocks::MockCancellationService),
        Arc::new(mocks::MockEventBusService::new()),
        // No audit here: `with_audit_service` sets it on the service, and that is the same seam
        // the production wiring uses.
        None,
        Arc::new(mocks::MockBudgetService),
        None,
    )
    .with_audit_service(Arc::new(CapturingAuditService { captured }))
    .with_sequence_policy(policy)
}

/// A sequence-policy double that records which plan-time gate ran.
///
/// The gates run *before* `build_graph`, so "no gate was consulted" is the assertion that bites when
/// `verify` is moved after the graph: the gate would already have been asked whether this runbook is
/// allowed, for a receipt that had not been admitted yet.
fn policy_recorder() -> super::RecordingPolicy {
    super::RecordingPolicy {
        order: std::sync::Mutex::new(Vec::new()),
    }
}

// ---------------------------------------------------------------------------
// Cases
// ---------------------------------------------------------------------------

/// A refusal that is the receipt's own, and nothing ran.
#[tokio::test]
async fn a_refused_receipt_dispatches_nothing_and_builds_no_envelope() {
    let repo = TempRepo::new();
    let calls = Arc::new(AtomicUsize::new(0));
    let captured: Arc<std::sync::Mutex<Option<audit_app::BuildEnvelopeInput>>> =
        Arc::new(std::sync::Mutex::new(None));
    let policy = Arc::new(policy_recorder());
    let orch = service(calls.clone(), captured.clone(), policy.clone()).await;

    // The receipt names a tree that is not this repository's, so `verify` refuses it on the rule it
    // is about — the workspace — rather than on schema, digest or structure.
    let err = orch
        .run_from_template(input(
            repo.root(),
            receipt_for("0000000000000000000000000000000000000000"),
        ))
        .await
        .expect_err("a receipt naming another tree must be refused");

    let (kinds, detail) = match &err {
        OrchestratorError::ReceiptRefused { kinds, detail } => (kinds.clone(), detail.clone()),
        other => panic!("the refusal must be the receipt's own variant, got: {other:?}"),
    };
    assert!(
        kinds.split(',').any(|k| k == "workspace_mismatch"),
        "the kind names the rule that fired: {kinds}"
    );
    assert!(
        detail.contains("0000000000000000000000000000000000000000"),
        "the sentence quotes the workspace the receipt claimed: {detail}"
    );

    assert_eq!(
        calls.load(Ordering::SeqCst),
        0,
        "verify runs before build_graph, so the executor must never be asked to run a graph"
    );
    assert!(
        policy.order.lock().unwrap().is_empty(),
        "verify runs before the plan-time gates, so a refused receipt consults no policy: {:?}",
        policy.order.lock().unwrap()
    );
    assert!(
        captured.lock().unwrap().is_none(),
        "a refused receipt builds no audit envelope: no record exists to summarise"
    );
}

/// The control: a verified receipt runs, and its digest reaches the envelope and the output.
///
/// This is what makes the case above an assertion rather than a coincidence — the same harness, the
/// same receipt fixture, one field different.
#[tokio::test]
async fn a_verified_receipt_runs_once_and_its_digest_reaches_the_envelope() {
    let repo = TempRepo::new();
    let identity = crate::receipt::workspace_identity(&repo.root()).expect("the fixture is a repo");
    let receipt = receipt_for(&identity);
    // The expectation is the verifier's own answer, computed here rather than read off the run:
    // `verify` is the function that decides a digest is admissible, so asserting the run recorded
    // *that* digest is the claim. Anything else would let the run define its own answer.
    let expected_digest = crate::receipt::verify(&receipt, &identity)
        .expect("the fixture receipt verifies")
        .digest()
        .to_string();
    assert_eq!(expected_digest.len(), 64, "a sha256 in hex");

    let calls = Arc::new(AtomicUsize::new(0));
    let captured: Arc<std::sync::Mutex<Option<audit_app::BuildEnvelopeInput>>> =
        Arc::new(std::sync::Mutex::new(None));
    let policy = Arc::new(policy_recorder());
    let orch = service(calls.clone(), captured.clone(), policy.clone()).await;

    let out = orch
        .run_from_template(input(repo.root(), receipt))
        .await
        .expect("a receipt naming this tree verifies");

    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "a verified receipt reaches the executor"
    );
    assert_eq!(
        *policy.order.lock().unwrap(),
        vec!["evaluate_plan", "evaluate_requirements"],
        "the gates ran for an admitted receipt — which is what makes the empty list above an \
         assertion rather than a harness that never consults them"
    );
    assert_eq!(
        out.receipt_digest.as_deref(),
        Some(expected_digest.as_str()),
        "RunOutput names the receipt the run was admitted under"
    );

    let envelope_input = captured
        .lock()
        .unwrap()
        .take()
        .expect("a run that executed builds an envelope");
    assert_eq!(
        envelope_input.receipt_digest.as_deref(),
        Some(expected_digest.as_str()),
        "the same digest reaches BuildEnvelopeInput, so the signed record names it too"
    );
    assert!(
        envelope_input.attestation_digest.is_none(),
        "no attestation exists yet: P3 writes the first one, and this field must not anticipate it"
    );
}

/// A run that declares no receipt is unchanged: no reference, and the executor runs.
///
/// The additive field cannot turn "the caller said nothing" into a refusal, and the envelope cannot
/// name a receipt that does not exist.
#[tokio::test]
async fn a_run_without_a_receipt_carries_no_reference() {
    let repo = TempRepo::new();
    let calls = Arc::new(AtomicUsize::new(0));
    let captured: Arc<std::sync::Mutex<Option<audit_app::BuildEnvelopeInput>>> =
        Arc::new(std::sync::Mutex::new(None));
    let policy = Arc::new(policy_recorder());
    let orch = service(calls.clone(), captured.clone(), policy.clone()).await;

    let mut input = input(
        repo.root(),
        receipt_for("0000000000000000000000000000000000000000"),
    );
    input.receipt = None;

    let out = orch
        .run_from_template(input)
        .await
        .expect("declaring nothing is not a refusal");

    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(out.receipt_digest, None);
    let envelope_input = captured.lock().unwrap().take().expect("envelope");
    assert_eq!(envelope_input.receipt_digest, None);
}
