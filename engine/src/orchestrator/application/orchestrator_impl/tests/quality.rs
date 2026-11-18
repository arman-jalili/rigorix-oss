//! The quality level's subject: what the steps recorded (#1585).
//!
//! The two producers used to invent the classifier's six booleans — `iteration > 2` in the
//! validation loop, `final_status != Failed` in the orchestrator. These tests pin the replacement:
//! the observed outcome decides the tier, and the thing that used to decide it does not.

use super::*;
use crate::dag_engine::domain::{TaskGraph, TaskNode};
use crate::quality_gates::domain::{ClaimSource, QualityLevel, RecordedOutcome};

/// One node and one result, joined into the outcome the classifier reads.
fn graph_with(tool: &str, intent: &str) -> (TaskGraph, uuid::Uuid) {
    let id = uuid::Uuid::new_v4();
    let mut graph = TaskGraph::new();
    graph
        .add_unchecked(TaskNode::new(
            id,
            "step".to_string(),
            tool.to_string(),
            vec![],
            intent.to_string(),
        ))
        .expect("adds");
    graph.seal().expect("seals");
    (graph, id)
}

fn result_for(id: uuid::Uuid, success: bool) -> TaskResult {
    TaskResult {
        node_id: id.to_string(),
        node_name: "step".to_string(),
        status: if success {
            TaskStatus::Success
        } else {
            TaskStatus::Failure
        },
        duration_ms: 1,
        output: None,
        error: None,
        failure_type: None,
        retry_attempts: 0,
        tool_used: None,
    }
}

#[test]
fn the_graph_names_the_tool_and_the_result_names_the_outcome() {
    let (graph, id) = graph_with("run_command", r#"{"command":"cargo test --workspace"}"#);
    let outcomes = OrchestratorServiceImpl::recorded_outcomes(&graph, &[result_for(id, true)]);

    assert_eq!(outcomes.len(), 1);
    assert_eq!(outcomes[0].tool, "run_command");
    assert_eq!(
        outcomes[0].subject.as_deref(),
        Some("cargo test --workspace")
    );
    assert!(outcomes[0].passed);
}

#[test]
fn flipping_the_recorded_status_changes_the_tier() {
    // The mutation the issue asks for, as a test: the *recorded outcome* is the subject, so changing
    // it changes the classification and nothing else does.
    let (graph, id) = graph_with("run_command", r#"{"command":"cargo test --workspace"}"#);

    let passed = OrchestratorServiceImpl::recorded_outcomes(&graph, &[result_for(id, true)]);
    let failed = OrchestratorServiceImpl::recorded_outcomes(&graph, &[result_for(id, false)]);

    assert_eq!(
        crate::quality_gates::application::service_impl::classify_from_outcomes(&passed).level,
        QualityLevel::Workspace
    );
    assert_eq!(
        crate::quality_gates::application::service_impl::classify_from_outcomes(&failed).level,
        QualityLevel::TargetedTests,
        "a failed workspace run is not evidence of a workspace level"
    );
}

#[test]
fn a_step_whose_command_was_not_recorded_cannot_claim_the_workspace_tier() {
    let (graph, id) = graph_with("run_command", "{}");
    let outcomes = OrchestratorServiceImpl::recorded_outcomes(&graph, &[result_for(id, true)]);

    assert_eq!(outcomes[0].subject, None);
    assert_eq!(
        outcomes[0].kind(),
        crate::quality_gates::domain::CheckKind::Other,
        "a step that recorded no command is not evidence about a test scope"
    );
}

#[test]
fn a_result_with_no_node_is_skipped_rather_than_guessed() {
    let (graph, _id) = graph_with("run_command", r#"{"command":"cargo test --workspace"}"#);
    let orphan = result_for(uuid::Uuid::new_v4(), true);

    let outcomes = OrchestratorServiceImpl::recorded_outcomes(&graph, &[orphan]);
    assert!(
        outcomes.is_empty(),
        "a result whose node is not in the graph names no tool, so it is evidence about nothing"
    );
}

#[test]
fn a_run_with_no_recorded_quality_step_claims_nothing_of_its_own() {
    // The whole point: no outcome means no engine-sourced level, so a receipt cannot stamp
    // `source: Engine` on a claim nobody measured.
    let classification =
        crate::quality_gates::application::service_impl::classify_from_outcomes(&[]);
    assert_eq!(classification.level, QualityLevel::TargetedTests);
    assert_eq!(classification.source, ClaimSource::Caller);
    assert!(classification.evidence.is_empty());
}

#[test]
fn the_evidence_names_the_steps_the_level_came_from() {
    // Both nodes first, then seal: a sealed graph takes no more nodes, which is the graph's own
    // invariant rather than something to work around here.
    let test_id = uuid::Uuid::new_v4();
    let lint_id = uuid::Uuid::new_v4();
    let mut graph = TaskGraph::new();
    graph
        .add_unchecked(TaskNode::new(
            test_id,
            "tests".to_string(),
            "run_command".to_string(),
            vec![],
            r#"{"command":"cargo test --workspace"}"#.to_string(),
        ))
        .expect("adds");
    graph
        .add_unchecked(TaskNode::new(
            lint_id,
            "lint".to_string(),
            "run_command".to_string(),
            vec![],
            r#"{"command":"cargo clippy -- -D warnings"}"#.to_string(),
        ))
        .expect("adds");
    graph.seal().expect("seals");

    let outcomes = OrchestratorServiceImpl::recorded_outcomes(
        &graph,
        &[result_for(test_id, true), result_for(lint_id, true)],
    );
    let classification =
        crate::quality_gates::application::service_impl::classify_from_outcomes(&outcomes);

    // A workspace test plus a lint is not MergeReady (no format, no audit) — and it must not be:
    // the tier the outcomes support is the tier it reports.
    assert_eq!(classification.level, QualityLevel::Workspace);
    assert_eq!(classification.source, ClaimSource::Engine);
    assert_eq!(classification.evidence, vec![test_id.to_string()]);
    assert!(
        !classification.evidence.contains(&lint_id.to_string()),
        "a lint that did not decide the tier is not evidence for it"
    );
}

#[test]
fn a_recorded_outcome_is_the_only_thing_the_classifier_reads() {
    // Guards against a regression to the old shape: the input is the outcomes, so there is no field
    // a caller could set to assert a level it did not observe.
    let input = ClassifyTestScopeInput::default();
    assert!(input.outcomes.is_empty());
    let classification =
        crate::quality_gates::application::service_impl::classify_from_outcomes(&input.outcomes);
    assert_eq!(classification.source, ClaimSource::Caller);
}

#[test]
fn the_recorded_outcome_type_carries_a_name_for_the_explanation() {
    let outcome = RecordedOutcome::new("n1", "run_command", true).with_name("workspace tests");
    assert_eq!(outcome.label(), "workspace tests (n1) — other");
}
