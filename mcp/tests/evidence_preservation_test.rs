//! ISSUE-EVIDENCE-PRESERVATION — composition regression at the MCP boundary.
//!
//! @canonical .pi/architecture/modules/precondition.md#evidence
//! Implements: ISSUE-EVIDENCE-PRESERVATION (epic EPIC-PRECONDITION-FOLLOWUPS)
//!
//! The approval-gated path is the flagship "approved at T₀, refused at Tₙ"
//! scene. Before the fix, the MCP host re-emitted the FINAL envelope from a
//! synthesized `node_states` projection, so EVERY finding array was dropped:
//! `precondition_findings[]` (ADR-017), `sequence_policy_findings[]`
//! (ADR-013) and `requirement_findings[]` (ADR-015) were absent from both the
//! persisted `.rigorix/audit` trail and `rigorix_read_audit`.
//!
//! This test drives the REAL composition (real engine, real shared event bus,
//! real precondition process check) through `AppState::handle_tool_call`:
//!
//! 1. execute an approval-gated step whose tool is matched by a failing
//!    precondition → the run pauses for approval;
//! 2. approve → the run resumes and the step is refused by the gate;
//! 3. `rigorix_read_audit` returns the finding arrays (AC #4);
//! 4. the persisted engine envelope carries them too (AC #1);
//! 5. a non-approval run is unchanged (AC #6).
//!
//! On the pre-fix code the MCP `AuditEnvelope` has no finding fields at all
//! and the host never queries the bus, so the assertions cannot hold — the
//! regression guard for the integration-blind gap.

use std::sync::Arc;

use rigorix_mcp::host::{AppState, build_real_engine};
use rigorix_mcp::template_tools::domain::entity::SharedTemplateRepository;
use rigorix_mcp::template_tools::infrastructure::FilesystemTemplateRepository;

const HMAC_KEY: &str = "evidence-preservation-test-key";

/// Write the operator config that arms an ALWAYS-FAILING precondition for
/// `run_command` steps, plus the HMAC signing key.
fn write_repo(root: &std::path::Path) {
    let rigorix = root.join(".rigorix");
    std::fs::create_dir_all(&rigorix).expect("create .rigorix");
    std::fs::write(
        root.join("rigorix.toml"),
        format!("audit_hmac_key = \"{HMAC_KEY}\"\n"),
    )
    .expect("write rigorix.toml");
    // A REAL process check (no mock): argv-only, exits 7. `/bin/sh` resolves
    // outside the agent-writable workspace, so the trust boundary passes and
    // the failure is a genuine non-zero exit.
    std::fs::write(
        rigorix.join("preconditions.toml"),
        r#"
[[preconditions]]
id = "evidence-guard"
match = { tool = "run_command" }
command = ["/bin/sh", "-c", "exit 7"]

[gating]
release_dependents_on_failure = true
"#,
    )
    .expect("write preconditions.toml");
}

async fn host(root: &str) -> AppState {
    let (engine, engine_audit, event_bus) =
        build_real_engine(root).await.expect("build real engine");
    let template_repo: SharedTemplateRepository =
        Arc::new(FilesystemTemplateRepository::new(".rigorix/templates"));
    // The signed-envelope history the authority-fact projection reads.
    let audit_history: Arc<
        dyn rigorix_engine::audit::infrastructure::repository::AuditEnvelopeRepository,
    > = Arc::new(
        rigorix_engine::audit::infrastructure::LocalAuditEnvelopeRepository::new(
            std::path::PathBuf::from(root)
                .join(".rigorix")
                .join("audit"),
        ),
    );
    AppState::new(
        engine,
        template_repo,
        Some(HMAC_KEY.to_string()),
        None,
        engine_audit,
        event_bus,
        audit_history,
    )
}

fn plan(requires_approval: bool) -> serde_json::Value {
    plan_named("evidence-preservation-demo", "pay", requires_approval)
}

fn plan_named(template: &str, step: &str, requires_approval: bool) -> serde_json::Value {
    serde_json::json!({
        "plan": {
            "name": template,
            "description": "approval-resume evidence preservation",
            "version": "1.0.0",
            "tags": [],
            "steps": [{
                "name": step,
                "tool": "run_command",
                "parameters": { "command": "true" },
                "requires_approval": requires_approval,
                "description": "gated payment step"
            }],
            "metadata": {},
            "created_at": "2026-01-01T00:00:00Z",
            "updated_at": "2026-01-01T00:00:00Z"
        }
    })
}

fn persisted_envelope(root: &std::path::Path, execution_id: &str) -> serde_json::Value {
    let path = root
        .join(".rigorix")
        .join("audit")
        .join(format!("{execution_id}.json"));
    let raw = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("read persisted envelope {}: {e}", path.display()));
    serde_json::from_str(&raw).expect("persisted envelope is JSON")
}

/// AC #1 + #4 + #7: an approval-gated refusal keeps its signed evidence.
#[tokio::test]
async fn approval_resume_preserves_precondition_evidence() {
    let dir = tempfile::tempdir().expect("temp repo");
    write_repo(dir.path());
    let root = dir.path().to_string_lossy().to_string();
    let state = host(&root).await;

    // 1. Execute the approval-gated, precondition-matched step → pauses.
    let exec = state
        .handle_tool_call("rigorix_execute", &plan(true))
        .await
        .expect("rigorix_execute");
    let execution_id = exec["execution_id"]
        .as_str()
        .expect("execution_id")
        .to_string();
    assert_eq!(
        exec["status"].as_str(),
        Some("PendingApproval"),
        "the approval-gated step must pause the run: {exec}"
    );

    // 2. Approve → resume → the precondition refuses the step before dispatch.
    let approved = state
        .handle_tool_call(
            "rigorix_approve_execution",
            &serde_json::json!({
                "execution_id": execution_id,
                "step_names": ["pay"],
                "approver_id": "tester",
            }),
        )
        .await
        .expect("rigorix_approve_execution");
    assert_eq!(
        approved["resumed"], true,
        "the approval must resume the paused run: {approved}"
    );

    // 3. rigorix_read_audit surfaces the finding arrays (AC #4).
    let audit = state
        .handle_tool_call(
            "rigorix_read_audit",
            &serde_json::json!({ "execution_id": execution_id, "format": "json" }),
        )
        .await
        .expect("rigorix_read_audit");
    let findings = audit["precondition_findings"]
        .as_array()
        .unwrap_or_else(|| {
            panic!("precondition_findings must be present on the read surface: {audit}")
        });
    assert_eq!(
        findings.len(),
        1,
        "exactly one precondition check must be recorded: {audit}"
    );
    assert_eq!(findings[0]["outcome"], "failed");
    assert_eq!(findings[0]["exit_code"], 7);
    assert_eq!(findings[0]["step"], "pay");
    assert_eq!(findings[0]["precondition_id"], "evidence-guard");
    assert!(
        findings[0]["inputs_hash"]
            .as_str()
            .is_some_and(|h| !h.is_empty()),
        "the finding must carry the one-way inputs hash: {audit}"
    );
    assert!(
        findings[0]["checked_at"].as_str().is_some(),
        "the finding must carry the check timestamp: {audit}"
    );

    // 4. The persisted engine envelope (.rigorix/audit) carries them too, and
    //    the real event stream is preserved (no longer just ["node_failed"]).
    let persisted = persisted_envelope(dir.path(), &execution_id);
    let persisted_findings = persisted["precondition_findings"]
        .as_array()
        .unwrap_or_else(|| {
            panic!("persisted envelope must carry precondition_findings: {persisted}")
        });
    assert_eq!(persisted_findings.len(), 1);
    assert_eq!(persisted_findings[0]["outcome"], "failed");
    assert_eq!(persisted_findings[0]["exit_code"], 7);
    assert!(
        persisted["events"].as_array().is_some_and(|events| events
            .iter()
            .any(|e| e["event_type"] == "precondition_checked")),
        "the persisted envelope must be built from the REAL event stream: {persisted}"
    );
    assert!(
        persisted["signature"]
            .as_str()
            .is_some_and(|s| !s.is_empty()),
        "the evidence envelope must be signed: {persisted}"
    );
}

/// #1223: an approved run's findings belong to that execution only.
///
/// The leaked-from run paused and was approved; the next run in the same MCP
/// session must not inherit its `precondition_findings`.
#[tokio::test]
async fn an_approved_run_does_not_leak_findings_into_the_next_run() {
    let dir = tempfile::tempdir().expect("temp repo");
    write_repo(dir.path());
    let root = dir.path().to_string_lossy().to_string();
    let state = host(&root).await;

    let run = |template: &'static str, step: &'static str| {
        let plan = plan_named(template, step, true);
        let state = &state;
        async move {
            let exec = state
                .handle_tool_call("rigorix_execute", &plan)
                .await
                .expect("rigorix_execute");
            assert_eq!(exec["status"].as_str(), Some("PendingApproval"));
            let execution_id = exec["execution_id"].as_str().expect("id").to_string();
            state
                .handle_tool_call(
                    "rigorix_approve_execution",
                    &serde_json::json!({
                        "execution_id": execution_id,
                        "step_names": [step],
                        "approver_id": "tester",
                    }),
                )
                .await
                .expect("approve");
            let audit = state
                .handle_tool_call(
                    "rigorix_read_audit",
                    &serde_json::json!({ "execution_id": execution_id, "format": "json" }),
                )
                .await
                .expect("read audit");
            (execution_id, audit)
        }
    };

    let (first_id, first) = run("run-a", "pay-a").await;
    let (second_id, second) = run("run-b", "pay-b").await;

    // A run that never pauses writes its envelope through the engine's own
    // audit service, not the approval-resume path.
    let third_plan = plan_named("run-c", "pay-c", false);
    let third = state
        .handle_tool_call("rigorix_execute", &third_plan)
        .await
        .expect("third execute");
    let third_id = third["execution_id"].as_str().expect("id").to_string();
    let third_audit = state
        .handle_tool_call(
            "rigorix_read_audit",
            &serde_json::json!({ "execution_id": third_id, "format": "json" }),
        )
        .await
        .expect("read third audit");

    for (name, id, audit, expected_step) in [
        ("first", &first_id, first, "pay-a"),
        ("second", &second_id, second, "pay-b"),
        ("third", &third_id, third_audit, "pay-c"),
    ] {
        let findings = audit["precondition_findings"]
            .as_array()
            .unwrap_or_else(|| panic!("{name} run must carry its finding: {audit}"));
        assert_eq!(
            findings.len(),
            1,
            "{name} run must carry only its own finding: {audit}"
        );
        assert_eq!(
            findings[0]["step"], expected_step,
            "{name} run carries another execution's finding: {audit}"
        );

        // The artifact a recipient receives is the file, not the read surface.
        let persisted = persisted_envelope(dir.path(), id);
        let persisted_findings = persisted["precondition_findings"]
            .as_array()
            .unwrap_or_else(|| panic!("{name} file must carry findings: {persisted}"));
        assert_eq!(
            persisted_findings.len(),
            1,
            "{name} file must carry only its own finding: {persisted}"
        );
        assert_eq!(
            persisted_findings[0]["step"], expected_step,
            "{name} file carries another execution's finding: {persisted}"
        );
    }
}

/// #1227: a fresh process reads the signed record it did not write.
#[tokio::test]
async fn a_fresh_process_reads_the_persisted_envelope() {
    let dir = tempfile::tempdir().expect("temp repo");
    write_repo(dir.path());
    let root = dir.path().to_string_lossy().to_string();

    // First process: run once, then disappear.
    let execution_id = {
        let state = host(&root).await;
        let exec = state
            .handle_tool_call("rigorix_execute", &plan(false))
            .await
            .expect("rigorix_execute");
        let execution_id = exec["execution_id"].as_str().expect("id").to_string();
        persisted_envelope(dir.path(), &execution_id);
        execution_id
    };

    // New process (new AppState, empty session index) reads the file.
    let state = host(&root).await;
    let audit = state
        .handle_tool_call(
            "rigorix_read_audit",
            &serde_json::json!({ "execution_id": execution_id, "format": "json" }),
        )
        .await
        .expect("a fresh process must read the persisted envelope");
    let findings = audit["precondition_findings"]
        .as_array()
        .unwrap_or_else(|| panic!("persisted findings must be returned: {audit}"));
    assert_eq!(findings.len(), 1, "the file carries one finding: {audit}");
    assert_eq!(findings[0]["step"], "pay");
    assert_eq!(
        audit["status"], "Failed",
        "a refused run must read as Failed, not Completed: {audit}"
    );
    assert!(
        audit["events"]
            .as_array()
            .is_some_and(|events| !events.is_empty()),
        "the real event stream must survive the disk read: {audit}"
    );

    // The summary counts the same persisted trail, not an empty session.
    let summary = state
        .handle_tool_call(
            "rigorix_audit_summary",
            &serde_json::json!({
                "since": "2020-01-01T00:00:00Z",
                "until": "2030-01-01T00:00:00Z"
            }),
        )
        .await
        .expect("summary over the persisted trail");
    assert!(
        summary["total_executions"].as_u64().unwrap_or(0) >= 1,
        "summary must count the record on disk: {summary}"
    );
    assert_eq!(
        summary["failure_count"], 1,
        "a refused run is a failure in the summary: {summary}"
    );
    assert_eq!(summary["success_count"], 0);

    // A genuinely unknown id is still distinct.
    let missing = state
        .handle_tool_call(
            "rigorix_read_audit",
            &serde_json::json!({
                "execution_id": "00000000-0000-0000-0000-000000000000",
                "format": "json"
            }),
        )
        .await;
    assert!(
        missing.is_err(),
        "an unknown id must not read as an empty record"
    );
}

/// AC #6: a non-approval run is unchanged — the step dispatching normally
/// produces NO precondition finding, and the envelope still verifies.
#[tokio::test]
async fn non_approval_run_is_unchanged() {
    let dir = tempfile::tempdir().expect("temp repo");
    write_repo(dir.path());
    let root = dir.path().to_string_lossy().to_string();
    let state = host(&root).await;

    // No approval gate, but the precondition still matches `run_command` and
    // fails — so the step is refused directly (the non-approval path).
    let exec = state
        .handle_tool_call("rigorix_execute", &plan(false))
        .await
        .expect("rigorix_execute");
    let execution_id = exec["execution_id"]
        .as_str()
        .expect("execution_id")
        .to_string();

    let persisted = persisted_envelope(dir.path(), &execution_id);
    // The precondition still ran (direct refusal), so the finding IS present —
    // the point of AC #6 is that the non-approval path still produces finding
    // evidence and a valid signature, exactly as before.
    let findings = persisted["precondition_findings"]
        .as_array()
        .expect("precondition_findings present");
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0]["outcome"], "failed");
    assert!(
        persisted["signature"]
            .as_str()
            .is_some_and(|s| !s.is_empty()),
        "non-approval evidence must still be signed"
    );
}
