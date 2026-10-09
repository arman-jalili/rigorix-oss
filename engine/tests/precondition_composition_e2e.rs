//! ACT-3: end-to-end composition reachability test for the ADR-017 gate.
//!
//! @canonical .pi/architecture/modules/precondition.md#acceptance-criteria
//! Implements: ISSUE-PF-ACT-3 — prove the factory-armed precondition gate is
//!   reachable from operator config and produces refusal + envelope evidence.
//! Issue: #969 (epic EPIC-PRECONDITION-FOLLOWUPS)
//!
//! Every prior test exercised `PreconditionDispatchGate` in isolation or via a
//! manual `with_precondition_gate` call. This test builds the executor the way
//! production does — `ParallelExecutionFactoryImpl::create` with a
//! `PreconditionSetup::from_env` built from a real fixture file — and proves:
//!
//! 1. a matching step whose real process check exits non-zero is refused and
//!    its tool is never called (marker file never appears);
//! 2. a `precondition_checked` event is published and the audit envelope
//!    derives `precondition_findings[]` (outcome / exit_code / inputs_hash /
//!    checked_at) from it;
//! 3. with no config file the run dispatches unchanged (fail-open-absent).
//!
//! On the pre-ACT codebase this file does not compile (`precondition` is not a
//! `ParallelExecutionFactoryConfig` field), which is the regression guard for
//! the dormant-gate gap.

use std::collections::HashMap;
use std::sync::Arc;

use rigorix_engine::audit::application::dto::BuildEnvelopeInput;
use rigorix_engine::audit::application::envelope_factory_impl::AuditEnvelopeFactoryImpl;
use rigorix_engine::audit::application::factory::AuditEnvelopeFactory;
use rigorix_engine::audit::domain::{EventStatus, ExecutionEventRef};
use rigorix_engine::dag_engine::domain::{TaskGraph, TaskNode};
use rigorix_engine::event_system::application::dto::DrainPersistedInput;
use rigorix_engine::event_system::application::event_bus_service_impl::EventBusServiceImpl;
use rigorix_engine::event_system::application::service::EventBusService;
use rigorix_engine::event_system::domain::ExecutionEvent;
use rigorix_engine::execution_engine::application::dto::ExecuteGraphInput;
use rigorix_engine::execution_engine::application::factory::{
    ParallelExecutionFactory, ParallelExecutionFactoryConfig,
};
use rigorix_engine::execution_engine::application::factory_impl::ParallelExecutionFactoryImpl;
use rigorix_engine::execution_engine::application::service::ParallelExecutionService;
use rigorix_engine::execution_engine::domain::{
    ExecutionResult, NodeStatus, ParallelExecutorConfig,
};
use rigorix_engine::precondition::PreconditionSetup;
use uuid::Uuid;

const PRECONDITION_ID: &str = "reachability-guard";

fn write_precondition_config(repo_root: &std::path::Path) {
    let rigorix = repo_root.join(".rigorix");
    std::fs::create_dir_all(&rigorix).expect("create .rigorix");
    // A REAL process check (no mock): argv-only, exits 1. `/bin/sh` resolves
    // outside the agent-writable workspace, so the trust boundary passes and
    // the failure is a genuine non-zero exit (not a spawn/boundary error).
    let config = r#"
[[preconditions]]
id = "reachability-guard"
match = { tool = "run_command" }
command = ["/bin/sh", "-c", "exit 1"]

[gating]
release_dependents_on_failure = true
"#;
    std::fs::write(rigorix.join("preconditions.toml"), config).expect("write config");
}

async fn executor_with(
    repo_root: &std::path::Path,
    event_bus: Arc<dyn EventBusService>,
    with_config: bool,
) -> Box<dyn ParallelExecutionService> {
    let precondition = if with_config {
        Some(
            PreconditionSetup::from_env(repo_root)
                .expect("from_env")
                .expect("fixture present"),
        )
    } else {
        None
    };
    ParallelExecutionFactoryImpl::new()
        .create(ParallelExecutionFactoryConfig {
            executor_config: ParallelExecutorConfig::default(),
            event_bus: Some(event_bus),
            precondition,
            ..Default::default()
        })
        .await
        .expect("factory create")
}

fn single_node_graph(node_id: Uuid, marker: &std::path::Path) -> TaskGraph {
    let mut graph = TaskGraph::new();
    graph
        .add_unchecked(TaskNode::new(
            node_id,
            "pay",
            "run_command",
            vec![],
            format!("touch {}", marker.display()),
        ))
        .expect("add node");
    graph.seal().expect("seal");
    graph
}

async fn run(
    executor: &dyn ParallelExecutionService,
    dag_id: Uuid,
    graph: TaskGraph,
) -> ExecutionResult {
    executor
        .execute_graph(ExecuteGraphInput {
            dag_id,
            graph: Some(graph),
            config_override: None,
        })
        .await
        .expect("execute_graph")
        .result
}

fn envelope_input(execution_id: Uuid, events: Vec<ExecutionEventRef>) -> BuildEnvelopeInput {
    BuildEnvelopeInput {
        execution_id,
        template_id: "precondition-reachability".to_string(),
        planning_prompt: "reachability".to_string(),
        events,
        source: Some("integration-test".to_string()),
        repository: None,
        author: None,
        identity: None,
        effect_key: None,
        producer_id: None,
        sequence: None,
        prev_hash: None,
        history_policy: None,
        total_tokens: 0,
        duration_ms: 0,
        git_commit: None,
        git_branch: None,
        model_version: None,
        planning_prompt_content: None,
        file_paths: vec![],
        metadata: None,
        sign: true,
        scoring_results: HashMap::new(),
    }
}

#[tokio::test]
async fn armed_gate_refuses_matching_step_and_records_evidence() {
    let repo = tempfile::tempdir().expect("repo tempdir");
    write_precondition_config(repo.path());

    let marker = repo.path().join("tool-ran.marker");
    let _ = std::fs::remove_file(&marker);
    let node_id = Uuid::new_v4();
    let dag_id = Uuid::new_v4();
    let event_bus: Arc<dyn EventBusService> = Arc::new(EventBusServiceImpl::default());
    let executor = executor_with(repo.path(), Arc::clone(&event_bus), true).await;

    let result = run(
        executor.as_ref(),
        dag_id,
        single_node_graph(node_id, &marker),
    )
    .await;

    // 1. Refused before dispatch; the step's tool is never called.
    assert!(
        !marker.exists(),
        "the denied step's tool must never run (no marker file)"
    );
    assert_eq!(result.failed_count, 1);
    let task = result.node_results.get(&node_id).expect("node result");
    assert_eq!(task.failure_type.as_deref(), Some("precondition_denied"));
    assert_eq!(
        result
            .execution_states
            .get(&node_id)
            .expect("node state")
            .status,
        NodeStatus::Failed
    );

    // 2. Evidence: the composed run published a `precondition_checked` event.
    let drained = event_bus
        .drain_persisted(DrainPersistedInput { clear: true })
        .await
        .expect("drain persisted events");
    let event_refs: Vec<ExecutionEventRef> = drained
        .events
        .iter()
        .filter_map(|persisted| match &persisted.event {
            ExecutionEvent::PreconditionChecked {
                execution_id,
                precondition_id,
                step,
                outcome,
                exit_code,
                inputs_hash,
                summary,
                check_digest,
                authority_digest,
                check_writable,
                attribution,
                timestamp,
            } => Some(ExecutionEventRef {
                event_type: "precondition_checked".to_string(),
                summary: summary.clone(),
                occurred_at: *timestamp,
                correlation_id: Some(*execution_id),
                status: EventStatus::Failure,
                payload: Some(serde_json::json!({
                    "precondition_id": precondition_id,
                    "step": step,
                    "outcome": outcome,
                    "exit_code": exit_code,
                    "inputs_hash": inputs_hash,
                    "checked_at": timestamp.to_rfc3339(),
                    "summary": summary,
                    "check_digest": check_digest,
                    "authority_digest": authority_digest,
                    "check_writable": check_writable,
                    "attribution": attribution,
                })),
            }),
            _ => None,
        })
        .collect();
    assert_eq!(
        event_refs.len(),
        1,
        "exactly one precondition check must be recorded"
    );

    // 3. The signed audit envelope derives `precondition_findings[]`.
    let envelope = AuditEnvelopeFactoryImpl::new(Some("reachability-test-key".to_string()))
        .build_envelope(envelope_input(dag_id, event_refs))
        .await
        .expect("build envelope");
    assert!(
        envelope.signature.is_some(),
        "the evidence envelope must be signed"
    );
    assert_eq!(envelope.precondition_findings.len(), 1);
    let finding = &envelope.precondition_findings[0];
    assert_eq!(finding.precondition_id, PRECONDITION_ID);
    assert_eq!(finding.step, "pay");
    assert_eq!(finding.outcome, "failed");
    assert_eq!(finding.exit_code, Some(1));
    assert!(
        !finding.inputs_hash.is_empty(),
        "the finding must carry the one-way inputs hash"
    );
    assert!(finding.checked_at.timestamp() > 0);
}

#[tokio::test]
async fn absent_config_dispatches_unchanged() {
    let repo = tempfile::tempdir().expect("repo tempdir");
    let marker = repo.path().join("tool-ran.marker");
    let _ = std::fs::remove_file(&marker);
    let node_id = Uuid::new_v4();
    let dag_id = Uuid::new_v4();
    let event_bus: Arc<dyn EventBusService> = Arc::new(EventBusServiceImpl::default());
    let executor = executor_with(repo.path(), Arc::clone(&event_bus), false).await;

    let result = run(
        executor.as_ref(),
        dag_id,
        single_node_graph(node_id, &marker),
    )
    .await;

    // Fail-open-absent: no config => no gate => the step dispatches normally.
    assert!(
        marker.exists(),
        "without a config file the step must dispatch and run its tool"
    );
    assert_eq!(result.completed_count, 1);
    assert_eq!(result.failed_count, 0);

    let drained = event_bus
        .drain_persisted(DrainPersistedInput { clear: true })
        .await
        .expect("drain persisted events");
    assert!(
        !drained
            .events
            .iter()
            .any(|e| matches!(e.event, ExecutionEvent::PreconditionChecked { .. })),
        "no gate => no precondition evidence"
    );
}

/// Build a `precondition_checked` event ref carrying the ADR-017 attribution
/// fields (check digest, authority digest, boundary fact).
fn precondition_checked_ref(authority_digest: &str) -> ExecutionEventRef {
    let now = chrono::Utc::now();
    ExecutionEventRef {
        event_type: "precondition_checked".to_string(),
        summary: "authority check".to_string(),
        occurred_at: now,
        correlation_id: Some(Uuid::new_v4()),
        status: EventStatus::Failure,
        payload: Some(serde_json::json!({
            "precondition_id": "beneficiary-authorized",
            "step": "payout_execute",
            "outcome": "failed",
            "exit_code": 3,
            "inputs_hash": "sha256:inputs",
            "checked_at": now.to_rfc3339(),
            "summary": "denied",
            "check_digest": "sha256:check",
            "authority_digest": authority_digest,
            "check_writable": true,
        })),
    }
}

/// #987: a changed authority is visible on the signed envelope. The finding
/// carries the authority digest, and mutating it changes the canonical bytes
/// (hence the HMAC) an outsider verifies.
#[tokio::test]
async fn authority_tampering_is_visible_on_the_signed_envelope() {
    let dag_id = Uuid::new_v4();
    let envelope = AuditEnvelopeFactoryImpl::new(Some("attribution-test-key".to_string()))
        .build_envelope(envelope_input(
            dag_id,
            vec![precondition_checked_ref("sha256:authority-good")],
        ))
        .await
        .expect("build envelope");

    let finding = &envelope.precondition_findings[0];
    assert_eq!(
        finding.authority_digest.as_deref(),
        Some("sha256:authority-good"),
        "the signed finding must bind the authority digest"
    );
    assert_eq!(finding.check_digest.as_deref(), Some("sha256:check"));
    assert_eq!(finding.check_writable, Some(true));

    let before = AuditEnvelopeFactoryImpl::canonical_envelope_bytes(&envelope).expect("canonical");

    // A forged/changed authority changes the digest -> changes the signed bytes.
    let mut tampered = envelope.clone();
    tampered.precondition_findings[0].authority_digest = Some("sha256:authority-forged".into());
    let after = AuditEnvelopeFactoryImpl::canonical_envelope_bytes(&tampered).expect("canonical");
    assert_ne!(
        before, after,
        "a changed authority digest must change the signed bytes"
    );

    // Absent fields = pre-attribution envelope (additive; absent != tampered).
    let mut pre_attribution = envelope.clone();
    pre_attribution.precondition_findings[0].authority_digest = None;
    pre_attribution.precondition_findings[0].check_digest = None;
    pre_attribution.precondition_findings[0].check_writable = None;
    let stripped =
        AuditEnvelopeFactoryImpl::canonical_envelope_bytes(&pre_attribution).expect("canonical");
    assert_ne!(before, stripped);
}
