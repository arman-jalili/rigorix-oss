//! #1655 — an overwrite that changes what governs a run is reported, and the previous definition is
//! recoverable.
//!
//! The audit's executed case: an operator template with `requires_approval = true` pauses a run; the
//! agent calls `rigorix_create_template` with `overwrite: true` and a plan whose step is `file_write`
//! with `requires_approval: false`; the same run completes with no human in the loop, the step's tool
//! changed, and nothing in the record says a gate was removed.
//!
//! The store's own report is the subject here — not the MCP tool's JSON — because a report assembled
//! from a request is how a record comes to attest what did not happen.

use rigorix_mcp::template_tools::domain::entity::TemplateRepository;
use rigorix_mcp::template_tools::domain::value::PlanTemplate;
use rigorix_mcp::template_tools::infrastructure::filesystem_repository::FilesystemTemplateRepository;
use serde_json::json;

fn plan(step_tool: &str, requires_approval: bool) -> PlanTemplate {
    PlanTemplate::from_json(json!({
        "name": "governed",
        "description": "a plan",
        "version": "1.0.0",
        "created_at": "2026-11-24T00:00:00Z",
        "updated_at": "2026-11-24T00:00:00Z",
        "steps": [
            {
                "name": "act",
                "tool": step_tool,
                "parameters": { "path": "out.txt" },
                "requires_approval": requires_approval,
                "description": "the step"
            }
        ]
    }))
    .expect("a valid plan")
}

#[tokio::test]
async fn an_overwrite_that_removes_an_approval_requirement_is_reported() {
    let dir = tempfile::tempdir().expect("tempdir");
    let repository = FilesystemTemplateRepository::new(dir.path());
    repository
        .create(plan("file_write", true), false)
        .await
        .expect("create");

    let report = repository
        .create_reporting(plan("file_write", false), true)
        .await
        .expect("overwrite");

    assert!(report.replaced_existing, "the file existed");
    assert!(
        report.approval_was_removed(),
        "dropping `requires_approval` is the change the audit measured: {:?}",
        report.changes
    );
    let summary = report.summary();
    assert!(summary.contains("requires_approval"), "{summary}");
    assert!(summary.contains("true -> false"), "{summary}");
    assert!(
        summary.contains("act"),
        "the report names the step: {summary}"
    );
}

#[tokio::test]
async fn an_overwrite_that_changes_a_step_tool_is_reported() {
    let dir = tempfile::tempdir().expect("tempdir");
    let repository = FilesystemTemplateRepository::new(dir.path());
    repository
        .create(plan("file_write", true), false)
        .await
        .expect("create");

    let report = repository
        .create_reporting(plan("run_command", true), true)
        .await
        .expect("overwrite");
    let changes = report.summary();
    assert!(
        changes.contains("`tool` file_write -> run_command"),
        "{changes}"
    );
}

#[tokio::test]
async fn the_previous_definition_is_recoverable_after_an_overwrite() {
    let dir = tempfile::tempdir().expect("tempdir");
    let repository = FilesystemTemplateRepository::new(dir.path());
    repository
        .create(plan("file_write", true), false)
        .await
        .expect("create");

    let report = repository
        .create_reporting(plan("file_write", false), true)
        .await
        .expect("overwrite");
    let kept = report
        .previous_definition
        .expect("the store names where the previous definition is");

    let bytes = std::fs::read_to_string(&kept).expect("the kept copy is readable");
    assert!(
        bytes.contains("requires_approval = true"),
        "the kept copy is the *previous* document, approval flag included: {bytes}"
    );

    let live = repository.get("governed").await.expect("the live document");
    assert!(
        !live.steps()[0].requires_approval(),
        "the live document is the new one"
    );
}

#[tokio::test]
async fn an_overwrite_that_changes_nothing_governance_relevant_is_silent() {
    let dir = tempfile::tempdir().expect("tempdir");
    let repository = FilesystemTemplateRepository::new(dir.path());
    repository
        .create(plan("file_write", true), false)
        .await
        .expect("create");

    let mut same = plan("file_write", true);
    // A description edit is not a boundary change and must not appear in the report: a report that
    // lists everything is a report nobody reads.
    same = PlanTemplate::from_json(json!({
        "name": "governed",
        "description": "a plan, re-described",
        "version": "1.0.0",
        "created_at": "2026-11-24T00:00:00Z",
        "updated_at": "2026-11-24T00:00:00Z",
        "steps": [
            {
                "name": "act",
                "tool": "file_write",
                "parameters": { "path": "out.txt" },
                "requires_approval": true,
                "description": "the step, re-described"
            }
        ]
    }))
    .expect("valid");

    let report = repository
        .create_reporting(same, true)
        .await
        .expect("overwrite");
    assert!(report.replaced_existing);
    assert_eq!(
        report.changes,
        Vec::new(),
        "a description edit is not a governance change: {:?}",
        report.changes
    );
}

#[tokio::test]
async fn a_creating_write_reports_that_it_replaced_nothing() {
    let dir = tempfile::tempdir().expect("tempdir");
    let repository = FilesystemTemplateRepository::new(dir.path());
    let report = repository
        .create_reporting(plan("file_write", true), false)
        .await
        .expect("create");
    assert!(!report.replaced_existing);
    assert!(report.changes.is_empty());
    assert!(report.previous_definition.is_none());
}
