//! Process-level contract with a workflow (#1209).
//!
//! The parser test proves inputs parse; this runs the **built binary** the way
//! a workflow does — fixture `GITHUB_*` environment, fixture workspace,
//! `GITHUB_OUTPUT`/`GITHUB_STEP_SUMMARY` files — and asserts what the runner
//! would observe: exit code, emitted outputs, and the step summary.
//!
//! `status` mode is used because it exercises the real orchestrator and router
//! while staying offline: no LLM call, network, database, or GitHub token.

use std::fs;
#[allow(
    clippy::disallowed_types,
    reason = "this test's subject is the shipped binary, so spawning it is the assertion"
)]
use std::process::Command;

#[test]
#[allow(
    clippy::disallowed_types,
    reason = "this test's subject is the shipped binary, so spawning it is the assertion — not an engine dispatch sink"
)]
fn the_action_binary_emits_outputs_and_exits_zero() {
    let temp = tempfile::tempdir().expect("tempdir");
    let workspace = temp.path().join("workspace");
    fs::create_dir(&workspace).expect("workspace");
    let output_file = temp.path().join("github-output");
    let summary_file = temp.path().join("github-summary");
    fs::write(&output_file, "").expect("output file");
    fs::write(&summary_file, "").expect("summary file");

    let result = Command::new(env!("CARGO_BIN_EXE_rigorix-action"))
        .env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .env("HOME", temp.path())
        .env("GITHUB_WORKSPACE", &workspace)
        .env("GITHUB_EVENT_NAME", "workflow_dispatch")
        .env("GITHUB_OUTPUT", &output_file)
        .env("GITHUB_STEP_SUMMARY", &summary_file)
        .env("INPUT_MODE", "status")
        .env("INPUT_POST_PR_COMMENT", "false")
        .env("RIGORIX_LOG", "error")
        .output()
        .expect("run the action binary");

    assert!(
        result.status.success(),
        "the action must exit 0 in status mode; stderr:\n{}",
        String::from_utf8_lossy(&result.stderr)
    );

    let outputs = fs::read_to_string(&output_file).expect("read GITHUB_OUTPUT");
    let line = |key: &str| {
        outputs
            .lines()
            .find(|line| line.starts_with(&format!("{key}=")))
            .unwrap_or_else(|| panic!("missing output {key} in:\n{outputs}"))
    };
    assert_eq!(line("status"), "status=Success");
    assert_eq!(line("mode_used"), "mode_used=status");
    assert!(
        line("execution_id").len() > "execution_id=".len(),
        "a status run must report the execution id it inspected:\n{outputs}"
    );

    let summary = fs::read_to_string(&summary_file).expect("read GITHUB_STEP_SUMMARY");
    assert!(
        summary.contains("Rigorix — status"),
        "the step summary must name the resolved mode:\n{summary}"
    );
    assert!(
        summary.contains("Status: Completed"),
        "the step summary must report the orchestrator status:\n{summary}"
    );
}
