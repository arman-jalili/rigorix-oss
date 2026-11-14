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

/// A receipt that names `workspace` as the tree it applies to, for `execution_id`.
fn receipt_for(workspace: &str, execution_id: Uuid) -> Receipt {
    Receipt::new(
        execution_id,
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
        execution_id: Some(receipt.execution_id),
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
    let foreign = Uuid::new_v4();
    let err = orch
        .run_from_template(input(
            repo.root(),
            receipt_for("0000000000000000000000000000000000000000", foreign),
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
    let execution_id = Uuid::new_v4();
    let receipt = receipt_for(&identity, execution_id);
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
        receipt_for("0000000000000000000000000000000000000000", Uuid::new_v4()),
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

// ---------------------------------------------------------------------------
// P3: the attestation is sealed beside the repository, and referenced by digest
// ---------------------------------------------------------------------------

/// A key the store can sign with, and the bytes `verify_attestation` will be handed.
const KEY: &str = "attestation-test-key";

/// A sibling of the repository, unique to this fixture.
///
/// `temp_dir()/state` would be shared by every test in this file — the repository is
/// `temp_dir()/rigorix-receipt-gate-<pid>-<n>`, so its parent is the temporary directory itself.
/// A sibling named after the repository is still outside the workspace, and is one test's alone.
fn state_dir(repo: &TempRepo) -> std::path::PathBuf {
    let name = repo
        .path
        .file_name()
        .expect("the fixture names itself")
        .to_string_lossy()
        .to_string();
    repo.path.with_file_name(format!("{name}.state"))
}

/// The record is written once, outside the workspace, and the envelope references its digest.
#[tokio::test]
async fn a_verified_receipt_seals_one_attestation_outside_the_workspace() {
    let repo = TempRepo::new();
    let identity = crate::receipt::workspace_identity(&repo.root()).expect("the fixture is a repo");
    let execution_id = Uuid::new_v4();
    let receipt = receipt_for(&identity, execution_id);
    let receipt_digest = crate::receipt::verify(&receipt, &identity)
        .expect("verifies")
        .digest()
        .to_string();

    let calls = Arc::new(AtomicUsize::new(0));
    let captured: Arc<std::sync::Mutex<Option<audit_app::BuildEnvelopeInput>>> =
        Arc::new(std::sync::Mutex::new(None));
    let policy = Arc::new(policy_recorder());
    let orch = service(calls, captured.clone(), policy)
        .await
        .with_attestation_key(Some(KEY.to_string()))
        .with_attestation_state_dir(state_dir(&repo));

    let out = orch
        .run_from_template(input(repo.root(), receipt))
        .await
        .expect("a receipt naming this tree verifies");

    let digest = out
        .attestation_digest
        .clone()
        .expect("a keyed run seals an attestation");
    assert_eq!(digest.len(), 64, "a sha256 in hex");

    // One record, in the state directory, named by the execution.
    let path = state_dir(&repo)
        .join("attestations")
        .join(format!("{}.json", out.execution_id));
    let bytes = std::fs::read(&path).expect("the attestation file exists");
    // Not in the envelope store: that directory is inside the agent-writable tree, which is the
    // whole reason the record lives here instead (ADR-024 §6).
    assert!(
        !path.canonicalize().expect("the record exists").starts_with(
            repo.path
                .canonicalize()
                .expect("the fixture repository exists")
        ),
        "the attestation must not be inside the workspace: {}",
        path.display()
    );

    // The record verifies with the key, and it answers the receipt the run was admitted under.
    let verified = crate::receipt::verify_attestation(&bytes, KEY.as_bytes())
        .expect("the sealed record verifies");
    assert_eq!(verified.attestation().receipt_digest(), receipt_digest);
    assert_eq!(
        verified.attestation().digest().expect("canonical"),
        digest,
        "the digest the run reports is the digest of the bytes that were written"
    );
    // The step ran and the observation says so — a record of nothing would also verify.
    let observed = verified.attestation().steps();
    assert_eq!(
        observed.len(),
        1,
        "one step in the receipt, one observation"
    );
    assert_eq!(observed[0].step_id, "write");

    // And the envelope carries the reference rather than a copy.
    let envelope = captured.lock().unwrap().take().expect("envelope");
    assert_eq!(
        envelope.attestation_digest.as_deref(),
        Some(digest.as_str())
    );
    assert_eq!(
        envelope.receipt_digest.as_deref(),
        Some(receipt_digest.as_str())
    );
}

/// Without a key nothing is sealed — and the absence is visible, not silent.
#[tokio::test]
async fn without_a_key_no_attestation_is_written_and_no_digest_is_claimed() {
    let repo = TempRepo::new();
    let identity = crate::receipt::workspace_identity(&repo.root()).expect("the fixture is a repo");

    let calls = Arc::new(AtomicUsize::new(0));
    let captured: Arc<std::sync::Mutex<Option<audit_app::BuildEnvelopeInput>>> =
        Arc::new(std::sync::Mutex::new(None));
    let policy = Arc::new(policy_recorder());
    let orch = service(calls, captured.clone(), policy)
        .await
        // No key: the store refuses to write an unsigned record, so there is no record at all.
        .with_attestation_key(None)
        .with_attestation_state_dir(state_dir(&repo));

    let out = orch
        .run_from_template(input(repo.root(), receipt_for(&identity, Uuid::new_v4())))
        .await
        .expect("an unsigned deployment still runs the work");

    assert_eq!(
        out.attestation_digest, None,
        "an unsigned record cannot be verified by the only reader the module has, so none is claimed"
    );
    assert!(
        !state_dir(&repo).join("attestations").exists(),
        "nothing is written when nothing can be signed"
    );
    let envelope = captured.lock().unwrap().take().expect("envelope");
    assert_eq!(envelope.attestation_digest, None);
    assert!(
        envelope.receipt_digest.is_some(),
        "the receipt reference is still there: the declaration was verified even though nothing \
         was sealed"
    );
}

/// A state directory inside the workspace is refused, on resolved paths, before any write.
#[tokio::test]
async fn a_state_dir_inside_the_workspace_refuses_the_seal() {
    let repo = TempRepo::new();
    let identity = crate::receipt::workspace_identity(&repo.root()).expect("the fixture is a repo");
    // The string does not start with the repository, and the resolved path does — the rule is about
    // where the store is, not about how it was spelled.
    let inside = repo
        .path
        .join("sub")
        .join("..")
        .join(".rigorix")
        .join("state");

    let calls = Arc::new(AtomicUsize::new(0));
    let captured: Arc<std::sync::Mutex<Option<audit_app::BuildEnvelopeInput>>> =
        Arc::new(std::sync::Mutex::new(None));
    let policy = Arc::new(policy_recorder());
    let orch = service(calls, captured.clone(), policy)
        .await
        .with_attestation_key(Some(KEY.to_string()))
        .with_attestation_state_dir(inside.clone());

    let out = orch
        .run_from_template(input(repo.root(), receipt_for(&identity, Uuid::new_v4())))
        .await
        .expect("a refused store does not fail the run; it withholds the record");

    assert_eq!(out.attestation_digest, None);
    assert!(
        !inside.join("attestations").exists(),
        "the store refuses before it creates a directory: {}",
        inside.display()
    );
}

/// A receipt is for one execution, and a receipt for another is refused before anything runs.
#[tokio::test]
async fn a_receipt_for_another_execution_is_refused() {
    let repo = TempRepo::new();
    let identity = crate::receipt::workspace_identity(&repo.root()).expect("the fixture is a repo");
    // A receipt that names this tree — so the workspace check would pass — and a different run.
    let receipt = receipt_for(&identity, Uuid::new_v4());
    let this_run = Uuid::new_v4();

    let calls = Arc::new(AtomicUsize::new(0));
    let captured: Arc<std::sync::Mutex<Option<audit_app::BuildEnvelopeInput>>> =
        Arc::new(std::sync::Mutex::new(None));
    let policy = Arc::new(policy_recorder());
    let orch = service(calls.clone(), captured.clone(), policy.clone()).await;

    let mut input = input(repo.root(), receipt);
    input.execution_id = Some(this_run);

    let err = orch
        .run_from_template(input)
        .await
        .expect_err("a receipt for another execution is not this run's declaration");
    match &err {
        OrchestratorError::ReceiptRefused { kinds, detail } => {
            assert_eq!(kinds, "execution_id_mismatch", "{kinds}");
            assert!(
                detail.contains(&this_run.to_string()),
                "the refusal names the run it was handed to: {detail}"
            );
        }
        other => panic!("the refusal must be the receipt's own variant, got: {other:?}"),
    }
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert!(policy.order.lock().unwrap().is_empty());
    assert!(captured.lock().unwrap().is_none());
}

// ---------------------------------------------------------------------------
// P4: a seal is authoritative — the same observation is idempotent, a different one is refused
// ---------------------------------------------------------------------------

/// Re-running the same execution with the same declaration writes nothing and changes nothing.
#[tokio::test]
async fn the_same_observation_for_the_same_execution_is_already_sealed() {
    let repo = TempRepo::new();
    let identity = crate::receipt::workspace_identity(&repo.root()).expect("the fixture is a repo");
    let execution_id = Uuid::new_v4();

    let run = |state: std::path::PathBuf| {
        let repo_root = repo.root();
        let receipt = receipt_for(&identity, execution_id);
        async move {
            let calls = Arc::new(AtomicUsize::new(0));
            let captured: Arc<std::sync::Mutex<Option<audit_app::BuildEnvelopeInput>>> =
                Arc::new(std::sync::Mutex::new(None));
            let policy = Arc::new(policy_recorder());
            let orch = service(calls, captured, policy)
                .await
                .with_attestation_key(Some(KEY.to_string()))
                .with_attestation_state_dir(state);
            orch.run_from_template(input(repo_root, receipt)).await
        }
    };

    let first = run(state_dir(&repo)).await.expect("the first run seals");
    let path = state_dir(&repo)
        .join("attestations")
        .join(format!("{execution_id}.json"));
    let after_first = std::fs::read(&path).expect("the record exists");

    let second = run(state_dir(&repo))
        .await
        .expect("the second run seals the same record");

    assert_eq!(first.attestation_digest, second.attestation_digest);
    assert_eq!(
        std::fs::read(&path).expect("the record still exists"),
        after_first,
        "the same observation must not rewrite the seal, and must not change its bytes"
    );
}

/// A different observation for the same execution is refused, and the seal is left as it was.
#[tokio::test]
async fn a_second_digest_for_the_same_execution_is_refused_and_the_seal_stands() {
    let repo = TempRepo::new();
    let identity = crate::receipt::workspace_identity(&repo.root()).expect("the fixture is a repo");
    let execution_id = Uuid::new_v4();

    // First: the fixture receipt, one step.
    let calls = Arc::new(AtomicUsize::new(0));
    let captured: Arc<std::sync::Mutex<Option<audit_app::BuildEnvelopeInput>>> =
        Arc::new(std::sync::Mutex::new(None));
    let policy = Arc::new(policy_recorder());
    let first = service(calls, captured, policy)
        .await
        .with_attestation_key(Some(KEY.to_string()))
        .with_attestation_state_dir(state_dir(&repo))
        .run_from_template(input(repo.root(), receipt_for(&identity, execution_id)))
        .await
        .expect("the first run seals");
    let sealed = first.attestation_digest.clone().expect("sealed");
    let path = state_dir(&repo)
        .join("attestations")
        .join(format!("{execution_id}.json"));

    // Second: the same execution, a declaration with one more step — a different observation.
    let mut wider = receipt_for(&identity, execution_id);
    let mut extra = wider.steps[0].clone();
    extra.id = "write-more".to_string();
    wider.steps.push(extra);

    let calls = Arc::new(AtomicUsize::new(0));
    let captured: Arc<std::sync::Mutex<Option<audit_app::BuildEnvelopeInput>>> =
        Arc::new(std::sync::Mutex::new(None));
    let policy = Arc::new(policy_recorder());
    let second = service(calls, captured, policy)
        .await
        .with_attestation_key(Some(KEY.to_string()))
        .with_attestation_state_dir(state_dir(&repo))
        .run_from_template(input(repo.root(), wider))
        .await
        .expect("a refused seal does not fail the run; it withholds the second record");

    assert_eq!(
        second.attestation_digest, None,
        "the second record is refused, so no digest is claimed"
    );

    // The seal is exactly what it was, and it still verifies.
    let bytes = std::fs::read(&path).expect("the original seal is still there");
    let verified = crate::receipt::verify_attestation(&bytes, KEY.as_bytes())
        .expect("the original seal still verifies");
    assert_eq!(verified.attestation().digest().expect("canonical"), sealed);
    // And the variant is the ADR's own, produced by the store rather than merely declared by it.
    // Asserted at the store, because the run reports the refusal as an absent digest — a run that
    // logged "sealed" and returned `None` would satisfy the assertions above for the wrong reason.
    let store = crate::receipt::infrastructure::FileAttestationStore::new(
        state_dir(&repo),
        &repo.path,
        Some(KEY.to_string()),
    );
    let verified = crate::receipt::verify(&receipt_for(&identity, execution_id), &identity)
        .expect("the original declaration still verifies");
    let mut replay = crate::receipt::AttestationBuilder::from_verified(&verified).build(1, 0);
    match store.put(&mut replay) {
        Err(crate::receipt::infrastructure::AttestationStoreError::Refused(
            crate::receipt::ReceiptRefusal::SealedDigestChanged {
                execution_id: named,
                sealed,
                presented,
            },
        )) => {
            assert_eq!(named, execution_id.to_string());
            assert_eq!(sealed, first.attestation_digest.clone().expect("sealed"));
            assert_ne!(
                presented, sealed,
                "a different observation is a different digest"
            );
        }
        other => panic!("a re-seal must be refused with SealedDigestChanged, got: {other:?}"),
    }
}
