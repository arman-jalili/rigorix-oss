//! The Action's `quality_level` output carries the engine's classification (#1587).
//!
//! Three facts made this worth a test file rather than a line: the engine classifies a level after
//! every run, the Action has an output named `quality_level`, and between them sat two literals — so a
//! workflow gating on the output gated on a constant, and the PR comment printed "workspace" for a run
//! whose tests never executed.
//!
//! These tests fix both ends of the wire: what the Action publishes for a given classification, and
//! what the comment says when there is none.

use rigorix_actions::action_entrypoint::domain::ActionOutput;
use rigorix_actions::action_output::application::pr_comment_impl::PrCommentServiceImpl;
use rigorix_actions::action_output::application::quality::{
    QUALITY_LEVEL_OUTPUT, QUALITY_SOURCE_OUTPUT, write_quality_outputs,
};
use rigorix_actions::action_output::application::service::PrCommentService;
use rigorix_actions::action_output::domain::{ExecutionContext, ExecutionStatus};
use rigorix_actions::shared::github_client::GitHubClient;
use rigorix_engine::quality_gates::application::dto::ClassifyTestScopeOutput;
use rigorix_engine::quality_gates::domain::{ClaimSource, QualityLevel};

fn claim(level: QualityLevel, source: ClaimSource) -> ClassifyTestScopeOutput {
    ClassifyTestScopeOutput {
        level,
        explanation: "a workspace-wide test run reported success (workspace)".to_string(),
        source,
        evidence: vec!["n1".to_string()],
    }
}

fn context(quality_level: Option<&str>) -> ExecutionContext {
    ExecutionContext {
        execution_id: uuid::Uuid::new_v4(),
        status: ExecutionStatus::Completed,
        iterations: 1,
        max_iterations: 3,
        cumulative_tokens: 0,
        duration_ms: 1200,
        quality_level: quality_level.map(str::to_string),
        template_id: None,
        failure_count: 0,
        file_changes: vec![],
        execution_steps: vec![],
        metadata: Default::default(),
    }
}

/// The value crossing the wire is the engine's, not a literal: this asserts the output equals the
/// classification it was handed, for two different levels so the test cannot pass on a constant.
#[test]
fn the_output_equals_the_value_the_engine_returned() {
    let mut workspace = ActionOutput::success("done", None);
    let published = write_quality_outputs(
        &mut workspace,
        Some(&claim(QualityLevel::Workspace, ClaimSource::Engine)),
    );
    assert_eq!(published, vec![QUALITY_LEVEL_OUTPUT, QUALITY_SOURCE_OUTPUT]);
    assert_eq!(
        workspace
            .output_variables
            .get(QUALITY_LEVEL_OUTPUT)
            .map(String::as_str),
        Some("workspace")
    );
    assert_eq!(
        workspace
            .output_variables
            .get(QUALITY_SOURCE_OUTPUT)
            .map(String::as_str),
        Some("engine")
    );

    let mut merge_ready = ActionOutput::success("done", None);
    write_quality_outputs(
        &mut merge_ready,
        Some(&claim(QualityLevel::MergeReady, ClaimSource::Engine)),
    )
    .len();
    assert_eq!(
        merge_ready
            .output_variables
            .get(QUALITY_LEVEL_OUTPUT)
            .map(String::as_str),
        Some("merge_ready"),
        "a second level must move the output, or the first assertion proved nothing"
    );
}

/// A run with no classification publishes no key — not an empty one, not a default level.
#[test]
fn no_value_produces_no_output_key() {
    let mut output = ActionOutput::success("done", None);
    assert!(write_quality_outputs(&mut output, None).is_empty());

    assert!(!output.output_variables.contains_key(QUALITY_LEVEL_OUTPUT));
    assert!(!output.output_variables.contains_key(QUALITY_SOURCE_OUTPUT));
    assert!(
        output.output_variables.is_empty(),
        "the wire writes nothing at all when there is nothing to write: {:?}",
        output.output_variables
    );
}

#[tokio::test]
async fn the_comment_prints_the_received_level() {
    let service = PrCommentServiceImpl::new(GitHubClient::new("test-token"));
    let body = service
        .format_execution_summary(&context(Some("merge_ready")))
        .await
        .expect("the summary is formatted without network");

    assert!(body.contains("**Quality:** merge_ready"), "{body}");
    assert!(!body.contains("not reported"), "{body}");
}

#[tokio::test]
async fn the_comment_says_the_level_was_not_reported_when_there_is_none() {
    let service = PrCommentServiceImpl::new(GitHubClient::new("test-token"));
    let body = service
        .format_execution_summary(&context(None))
        .await
        .expect("the summary is formatted without network");

    assert!(
        body.contains("**Quality:** not reported by the engine"),
        "an absent claim must read as absence, not as a level: {body}"
    );
    assert!(
        !body.contains("workspace"),
        "the old literal must not survive: {body}"
    );
}
