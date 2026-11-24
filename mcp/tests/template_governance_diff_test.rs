//! #1655 + ADR-026 — an overwrite that reduces what is gated is **refused**, and one that does not is
//! **reported** with its previous definition kept.
//!
//! The audit's executed case: an operator template with `requires_approval = true` pauses a run; the
//! agent calls the template tool with `overwrite: true` and a plan whose step drops the approval; the
//! same run completes with no human in the loop, the step's tool changed, and nothing in the record says
//! a gate was removed.
//!
//! #1655 made that write *visible*. ADR-026 makes the three classifiable reductions a refusal, because
//! `.rigorix/templates` is already out of reach of `file_write` (any `.rigorix` segment is protected), so
//! the sanctioned tool is the only writer — and a sanctioned writer that performs a write the ordinary
//! writer is forbidden to perform is a bypass with a nicer name.
//!
//! The store's own report is the subject here — not the tool's JSON — because a report assembled from a
//! request is how a record comes to attest what did not happen.

use rigorix_mcp::template_tools::domain::entity::TemplateRepository;
use rigorix_mcp::template_tools::domain::error::TemplateError;
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

/// The two-step document: one approved step and one that is not, so a removal can be observed on a
/// document that still has a gate left.
fn two_steps(second_approval: bool) -> PlanTemplate {
    PlanTemplate::from_json(json!({
        "name": "governed",
        "description": "a plan",
        "version": "1.0.0",
        "created_at": "2026-11-24T00:00:00Z",
        "updated_at": "2026-11-24T00:00:00Z",
        "steps": [
            {
                "name": "act",
                "tool": "file_write",
                "parameters": { "path": "out.txt" },
                "requires_approval": true,
                "description": "the step"
            },
            {
                "name": "check",
                "tool": "file_read",
                "parameters": { "path": "out.txt" },
                "requires_approval": second_approval,
                "description": "the second step"
            }
        ]
    }))
    .expect("a valid plan")
}

fn without_second_step() -> PlanTemplate {
    PlanTemplate::from_json(json!({
        "name": "governed",
        "description": "a plan",
        "version": "1.0.0",
        "created_at": "2026-11-24T00:00:00Z",
        "updated_at": "2026-11-24T00:00:00Z",
        "steps": [
            {
                "name": "act",
                "tool": "file_write",
                "parameters": { "path": "out.txt" },
                "requires_approval": true,
                "description": "the step"
            }
        ]
    }))
    .expect("a valid plan")
}

/// What the refusal must say, in the words the reader needs: the step, the field, both values.
fn assert_refusal(error: TemplateError, expected: &[&str]) {
    let TemplateError::GovernanceDowngrade(message) = error else {
        panic!("expected a governance-downgrade refusal, got {error:?}");
    };
    for fragment in expected {
        assert!(
            message.contains(fragment),
            "the refusal must contain {fragment:?}: {message}"
        );
    }
}

#[tokio::test]
async fn an_overwrite_that_removes_an_approval_requirement_is_refused() {
    let dir = tempfile::tempdir().expect("tempdir");
    let repository = FilesystemTemplateRepository::new(dir.path());
    repository
        .create(plan("file_write", true), false)
        .await
        .expect("create");

    let error = repository
        .create_reporting(plan("file_write", false), true)
        .await
        .expect_err("dropping `requires_approval` is a downgrade");
    assert_refusal(error, &["act", "requires_approval", "drops"]);
}

#[tokio::test]
async fn a_refused_overwrite_writes_nothing_at_all() {
    let dir = tempfile::tempdir().expect("tempdir");
    let repository = FilesystemTemplateRepository::new(dir.path());
    repository
        .create(plan("file_write", true), false)
        .await
        .expect("create");
    let before = std::fs::read_to_string(dir.path().join("governed.toml")).expect("the file");

    let _ = repository
        .create_reporting(plan("file_write", false), true)
        .await
        .expect_err("refused");

    let after = std::fs::read_to_string(dir.path().join("governed.toml")).expect("the file");
    assert_eq!(
        before, after,
        "the template file is byte-identical after a refusal"
    );
    assert!(
        !dir.path().join(".versions").exists(),
        "a refused write leaves no version copy: a record of a change that did not happen is worse \
         than no record"
    );
}

#[tokio::test]
async fn an_overwrite_that_retargets_a_step_is_refused() {
    let dir = tempfile::tempdir().expect("tempdir");
    let repository = FilesystemTemplateRepository::new(dir.path());
    repository
        .create(plan("file_write", true), false)
        .await
        .expect("create");

    let error = repository
        .create_reporting(plan("run_command", true), true)
        .await
        .expect_err("a retargeted step is a different action wearing the old name");
    assert_refusal(error, &["act", "file_write", "run_command", "retargeted"]);
}

#[tokio::test]
async fn an_overwrite_that_removes_a_step_is_refused() {
    let dir = tempfile::tempdir().expect("tempdir");
    let repository = FilesystemTemplateRepository::new(dir.path());
    repository
        .create(two_steps(true), false)
        .await
        .expect("create");

    let error = repository
        .create_reporting(without_second_step(), true)
        .await
        .expect_err("a removed step takes its gate with it");
    assert_refusal(error, &["check", "removed"]);
}

#[tokio::test]
async fn adding_an_approval_is_applied_and_reported() {
    // The complement: a change that *raises* the bar is not a refusal, and the report still names it.
    let dir = tempfile::tempdir().expect("tempdir");
    let repository = FilesystemTemplateRepository::new(dir.path());
    repository
        .create(two_steps(false), false)
        .await
        .expect("create");

    let report = repository
        .create_reporting(two_steps(true), true)
        .await
        .expect("adding approval is allowed");
    assert!(report.replaced_existing);
    let summary = report.summary();
    assert!(summary.contains("check"), "{summary}");
    assert!(summary.contains("false -> true"), "{summary}");
    let live = repository.get("governed").await.expect("the live document");
    assert!(
        live.steps().iter().all(|step| step.requires_approval()),
        "the write happened"
    );
}

#[tokio::test]
async fn adding_a_step_is_applied_and_reported() {
    let dir = tempfile::tempdir().expect("tempdir");
    let repository = FilesystemTemplateRepository::new(dir.path());
    repository
        .create(plan("file_write", true), false)
        .await
        .expect("create");

    let report = repository
        .create_reporting(two_steps(true), true)
        .await
        .expect("adding a step is allowed");
    let summary = report.summary();
    assert!(
        summary.contains("(absent)") && summary.contains("check"),
        "an added step is reported as absent -> present: {summary}"
    );
}

#[tokio::test]
async fn an_allowed_overwrite_keeps_the_previous_definition() {
    let dir = tempfile::tempdir().expect("tempdir");
    let repository = FilesystemTemplateRepository::new(dir.path());
    repository
        .create(plan("file_write", false), false)
        .await
        .expect("create");

    // Adding an approval is the direction that is allowed, so the kept copy is the *weaker* document —
    // which is the useful one to have on disk when someone asks what changed.
    let report = repository
        .create_reporting(plan("file_write", true), true)
        .await
        .expect("allowed");
    let kept = report
        .previous_definition
        .expect("the store names where the previous definition is");

    let bytes = std::fs::read_to_string(&kept).expect("the kept copy is readable");
    assert!(
        bytes.contains("requires_approval = false"),
        "the kept copy is the *previous* document: {bytes}"
    );
    let live = repository.get("governed").await.expect("the live document");
    assert!(
        live.steps()[0].requires_approval(),
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

    // A description edit is not a boundary change and must not appear in the report: a report that
    // lists everything is a report nobody reads.
    let same = PlanTemplate::from_json(json!({
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
        .create_reporting(plan("file_write", false), false)
        .await
        .expect("create");
    assert!(!report.replaced_existing);
    assert!(report.changes.is_empty());
    assert!(report.previous_definition.is_none());
}

#[tokio::test]
async fn changing_parameters_is_reported_not_refused() {
    // ADR-026's explicit remainder: "this parameter widens the effect" is not readable from a string
    // pair without a per-tool capability model, so it is applied and named. A refusal here would be a
    // guess, and a guess that refuses legitimate edits is how a control gets switched off.
    let dir = tempfile::tempdir().expect("tempdir");
    let repository = FilesystemTemplateRepository::new(dir.path());
    repository
        .create(plan("run_command", true), false)
        .await
        .expect("create");

    let widened = PlanTemplate::from_json(json!({
        "name": "governed",
        "description": "a plan",
        "version": "1.0.0",
        "created_at": "2026-11-24T00:00:00Z",
        "updated_at": "2026-11-24T00:00:00Z",
        "steps": [
            {
                "name": "act",
                "tool": "run_command",
                "parameters": { "command": "chmod 777 out.txt" },
                "requires_approval": true,
                "description": "the step"
            }
        ]
    }))
    .expect("valid");

    let report = repository
        .create_reporting(widened, true)
        .await
        .expect("a parameter change is applied");
    let summary = report.summary();
    assert!(
        summary.contains("parameters") && summary.contains("chmod 777 out.txt"),
        "the change is named in the report: {summary}"
    );
    assert!(
        report.previous_definition.is_some(),
        "and the previous definition is kept, which is what makes the report checkable"
    );
}
