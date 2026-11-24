//! #1660 — every template action survives **dialect → plan → effect**, one test per action.
//!
//! The earlier attempt at this issue produced "the actions we remembered": a suite that compares the
//! conversion with a hand-built expectation covers the actions somebody thought of, which is the defect
//! being fixed, renamed. So this harness has three properties:
//!
//! 1. **The document is the input.** Each case writes a real `[[nodes]]` dialect document into the
//!    store and reads it back through `repository.get`, which is the production read path — parse, then
//!    `action_to_step`, then a `PlanTemplate`. Nothing here calls the converter directly.
//! 2. **The effect is the assertion.** Each step's tool name and parameters are handed to the engine's
//!    **own tool implementation** in a temp workspace, and the test asserts what happened there: a file
//!    with the bytes, a staged path, a commit whose message is the message, a command that created
//!    something. A JSON comparison would pass for a step the dispatcher cannot execute.
//! 3. **The count is derived from the enum.** `EXERCISED` must equal the variants read out of
//!    `TemplateAction`'s own source, minus the one action the conversion refuses by design
//!    (`lsp_query`, #1651). A tenth action added to the dialect fails this file until it is driven.
//!
//! ## What this harness does not drive
//!
//! It calls the tool **implementations**, not `execute_tool` — the planner wrapper that adds budget
//! evaluation, the permission decision, hooks and the receipt. That wrapper needs a DAG, a run identity
//! and an assembled engine; it is covered by its own tests. What is proved here is the part #1660 is
//! about: that the document the operator writes reaches an effect in the workspace, and which one.

use std::collections::HashMap;

use rigorix_engine::tools::application::shell_policy::SandboxDeclaration;
use rigorix_engine::tools::application::{
    EditFileTool, FileAppendTool, FilePatchTool, FileReadTool, FileWriteTool, GitCommitTool,
    GitReadTool, GitStageTool, RunCommandTool,
};
use rigorix_engine::tools::domain::{Tool, ToolInput};
use rigorix_mcp::template_tools::domain::entity::TemplateRepository;
use rigorix_mcp::template_tools::domain::value::PlanTemplate;
use rigorix_mcp::template_tools::infrastructure::filesystem_repository::FilesystemTemplateRepository;
use serde_json::Value;

/// The action `type` strings this harness drives — the dialect's own vocabulary.
const EXERCISED: &[&str] = &[
    "file_read",
    "file_write",
    "file_append",
    "file_patch",
    "run_command",
    "tool_call",
    "git_read",
    "git_stage",
    "git_commit",
];

/// The one action the conversion **refuses** rather than converts (#1651): the dispatcher has no
/// `lsp_query` arm, so a step built from it could only fail at dispatch. Listed here so the coverage
/// assertion is a subtraction with a reason, not a number that happens to match.
const REFUSED_BY_DESIGN: &[&str] = &["lsp_query"];

/// A complete dialect document with one node carrying `action`.
fn document(name: &str, action: &str) -> String {
    format!(
        r#"schema_version = 1
id = "{name}"
name = "{name}"
description = "a node that must reach an effect"
version = "1.0.0"

[[nodes]]
id = "n1"
name = "act"
[nodes.action]
{action}
"#
    )
}

/// Write the dialect document into the store and read it back **through the production read path**.
async fn plan_from_dialect(dir: &std::path::Path, name: &str, action: &str) -> PlanTemplate {
    std::fs::write(dir.join(format!("{name}.toml")), document(name, action))
        .expect("write document");
    FilesystemTemplateRepository::new(dir)
        .get(name)
        .await
        .unwrap_or_else(|error| panic!("the dialect document must become a plan: {error}"))
}

/// The one step the returned plan carries, with the parameters the conversion produced.
fn only_step(plan: &PlanTemplate) -> (&str, HashMap<String, Value>) {
    let steps = plan.steps();
    assert_eq!(steps.len(), 1, "one node, one step");
    let params: HashMap<String, Value> = steps[0]
        .parameters()
        .as_object()
        .map(|object| {
            object
                .iter()
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect()
        })
        .unwrap_or_default();
    (steps[0].tool(), params)
}

fn input(params: HashMap<String, Value>) -> ToolInput {
    ToolInput::new(params)
}

/// A test fixture's own `git`, deliberately outside the sink policy: it seeds and then **reads back** the
/// repository the tools under test wrote to. The spawns being measured are the tools'; this one is the
/// ruler, and routing the ruler through the subject would make the assertion circular.
#[allow(clippy::disallowed_types)]
fn git(dir: &std::path::Path, args: &[&str]) -> String {
    let output = std::process::Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("GIT_AUTHOR_NAME", "conformance")
        .env("GIT_AUTHOR_EMAIL", "conformance@example.invalid")
        .env("GIT_COMMITTER_NAME", "conformance")
        .env("GIT_COMMITTER_EMAIL", "conformance@example.invalid")
        .output()
        .expect("git runs");
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn temp_git_repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    git(dir.path(), &["init", "-q", "-b", "main"]);
    // The identity goes in the repository's config, not the environment: the tool spawns git through the
    // child environment allowlist (#1653), so `GIT_AUTHOR_*` in this process would not reach it. A
    // repository without an identity genuinely cannot commit — that is git's rule, not a defect.
    git(dir.path(), &["config", "user.name", "conformance"]);
    git(
        dir.path(),
        &["config", "user.email", "conformance@example.invalid"],
    );
    std::fs::write(dir.path().join("tracked.txt"), "one\n").expect("seed file");
    git(dir.path(), &["add", "tracked.txt"]);
    git(dir.path(), &["commit", "-q", "-m", "seed"]);
    dir
}

#[tokio::test]
async fn file_write_puts_the_bytes_on_disk() {
    let store = tempfile::tempdir().expect("tempdir");
    let workspace = temp_git_repo();
    let plan = plan_from_dialect(
        store.path(),
        "w",
        r#"type = "file_write"
path = "written.txt"
content = "hello from the dialect""#,
    )
    .await;
    let (tool, params) = only_step(&plan);
    assert_eq!(
        tool, "file_write",
        "the step names the tool the dialect means"
    );

    let tool_impl = FileWriteTool::new(workspace.path().to_str().expect("utf8"));
    let result = tool_impl.execute(&input(params)).await.expect("executes");
    assert_eq!(result.exit_code, 0);
    let written = std::fs::read_to_string(workspace.path().join("written.txt")).expect("the file");
    assert_eq!(written, "hello from the dialect");
}

#[tokio::test]
async fn file_append_adds_to_the_existing_bytes() {
    let store = tempfile::tempdir().expect("tempdir");
    let workspace = temp_git_repo();
    let plan = plan_from_dialect(
        store.path(),
        "a",
        r#"type = "file_append"
path = "tracked.txt"
content = "two\n""#,
    )
    .await;
    let (tool, params) = only_step(&plan);
    assert_eq!(tool, "file_append");

    FileAppendTool::new(workspace.path().to_str().expect("utf8"))
        .execute(&input(params))
        .await
        .expect("executes");
    let content = std::fs::read_to_string(workspace.path().join("tracked.txt")).expect("the file");
    assert_eq!(content, "one\ntwo\n", "append adds, it does not replace");
}

#[tokio::test]
async fn file_patch_inserts_at_the_search_anchor() {
    let store = tempfile::tempdir().expect("tempdir");
    let workspace = temp_git_repo();
    let plan = plan_from_dialect(
        store.path(),
        "p",
        r#"type = "file_patch"
path = "tracked.txt"
search = "one"
insert = "zero\n"
before = true"#,
    )
    .await;
    let (tool, params) = only_step(&plan);
    assert_eq!(tool, "file_patch");
    assert_eq!(
        params.get("before").and_then(Value::as_bool),
        Some(true),
        "the conversion carries the direction of the insert"
    );

    FilePatchTool::new(workspace.path().to_str().expect("utf8"))
        .execute(&input(params))
        .await
        .expect("executes");
    let content = std::fs::read_to_string(workspace.path().join("tracked.txt")).expect("the file");
    assert!(
        content.starts_with("zero"),
        "insert-before puts the bytes before the anchor: {content:?}"
    );
}

#[tokio::test]
async fn file_read_returns_what_is_on_disk() {
    let store = tempfile::tempdir().expect("tempdir");
    let workspace = temp_git_repo();
    let plan = plan_from_dialect(
        store.path(),
        "r",
        r#"type = "file_read"
path = "tracked.txt""#,
    )
    .await;
    let (tool, params) = only_step(&plan);
    assert_eq!(tool, "file_read");

    let result = FileReadTool::new(workspace.path().to_str().expect("utf8"))
        .execute(&input(params))
        .await
        .expect("executes");
    assert!(
        result.output.contains("one"),
        "the read returns the content: {}",
        result.output
    );
}

#[tokio::test]
async fn run_command_executes_and_its_effect_is_visible() {
    let store = tempfile::tempdir().expect("tempdir");
    let workspace = temp_git_repo();
    let plan = plan_from_dialect(
        store.path(),
        "c",
        r#"type = "run_command"
command = "touch made-by-the-dialect.txt""#,
    )
    .await;
    let (tool, params) = only_step(&plan);
    assert_eq!(tool, "run_command");
    assert_eq!(
        params.get("command").and_then(Value::as_str),
        Some("touch made-by-the-dialect.txt"),
        "the command survives the conversion verbatim"
    );

    let result = RunCommandTool::new(
        workspace.path().to_str().expect("utf8"),
        vec!["touch"],
        SandboxDeclaration::container(),
    )
    .execute(&input(params))
    .await
    .expect("executes");
    assert_eq!(result.exit_code, 0, "{}", result.output);
    assert!(
        workspace.path().join("made-by-the-dialect.txt").exists(),
        "the command's effect is a file in the workspace: {}",
        result.output
    );
}

#[tokio::test]
async fn tool_call_is_the_call_the_document_wrote() {
    // The legacy demo dialect (#1529): the node names a tool and its arguments, so the step **is** that
    // call. This is the arm that keeps a named tool a real call rather than a translation.
    let store = tempfile::tempdir().expect("tempdir");
    let workspace = temp_git_repo();
    let plan = plan_from_dialect(
        store.path(),
        "t",
        r#"type = "tool_call"
tool = "file_write"
parameters = { path = "named.txt", content = "called by name" }"#,
    )
    .await;
    let (tool, params) = only_step(&plan);
    assert_eq!(tool, "file_write", "the named tool is the step's tool");
    assert_eq!(
        params.get("content").and_then(Value::as_str),
        Some("called by name"),
        "and its arguments are the document's arguments"
    );

    FileWriteTool::new(workspace.path().to_str().expect("utf8"))
        .execute(&input(params))
        .await
        .expect("executes");
    assert_eq!(
        std::fs::read_to_string(workspace.path().join("named.txt")).expect("the file"),
        "called by name"
    );
}

#[tokio::test]
async fn git_read_sees_the_repository() {
    let store = tempfile::tempdir().expect("tempdir");
    let workspace = temp_git_repo();
    let plan = plan_from_dialect(
        store.path(),
        "gr",
        r#"type = "git_read"
command = "status""#,
    )
    .await;
    let (tool, params) = only_step(&plan);
    assert_eq!(tool, "git_read");

    let result = GitReadTool::new(workspace.path().to_str().expect("utf8"))
        .execute(&input(params))
        .await
        .expect("executes");
    assert!(
        result.output.contains("main") || result.output.contains("branch"),
        "a status read reports the repository: {}",
        result.output
    );
}

#[tokio::test]
async fn git_stage_stages_the_path_the_step_names() {
    let store = tempfile::tempdir().expect("tempdir");
    let workspace = temp_git_repo();
    std::fs::write(workspace.path().join("staged.txt"), "stage me\n").expect("file");
    let plan = plan_from_dialect(
        store.path(),
        "gs",
        r#"type = "git_stage"
path = "staged.txt""#,
    )
    .await;
    let (tool, params) = only_step(&plan);
    assert_eq!(tool, "git_stage");

    GitStageTool::new(workspace.path().to_str().expect("utf8"))
        .execute(&input(params))
        .await
        .expect("executes");
    let staged = git(workspace.path(), &["diff", "--cached", "--name-only"]);
    assert!(
        staged.contains("staged.txt"),
        "the effect is an index entry, not a message: {staged}"
    );
}

#[tokio::test]
async fn git_commit_creates_the_commit_the_step_describes() {
    let store = tempfile::tempdir().expect("tempdir");
    let workspace = temp_git_repo();
    // `auto_stage` is `git add -u`: it stages modifications to **tracked** files. A new file is not
    // staged, which git reports as "nothing added to commit" on stdout and refuses on — so the case
    // modifies the tracked file, which is what the parameter promises.
    std::fs::write(workspace.path().join("tracked.txt"), "one\nmodified\n").expect("modify");
    let plan = plan_from_dialect(
        store.path(),
        "gc",
        r#"type = "git_commit"
message = "from the dialect"
auto_stage = true"#,
    )
    .await;
    let (tool, params) = only_step(&plan);
    assert_eq!(tool, "git_commit");
    assert_eq!(
        params.get("auto_stage").and_then(Value::as_bool),
        Some(true),
        "auto_stage is part of the call, so the step stages what it commits"
    );

    GitCommitTool::new(workspace.path().to_str().expect("utf8"))
        .execute(&input(params))
        .await
        .expect("executes");
    let subject = git(workspace.path(), &["log", "-1", "--format=%s"]);
    assert_eq!(
        subject.trim(),
        "from the dialect",
        "the commit exists, and its subject is the message"
    );
}

#[tokio::test]
async fn edit_file_replaces_the_string_the_step_names() {
    // `edit_file` is a dispatcher name with an engine implementation but no *dialect* action of its
    // own: a document reaches it through `tool_call`. Driving it here is the point — the dispatcher's
    // ninth tool is executed too, so "every action reaches an effect" is not nine-of-eight.
    let store = tempfile::tempdir().expect("tempdir");
    let workspace = temp_git_repo();
    let plan = plan_from_dialect(
        store.path(),
        "e",
        r#"type = "tool_call"
tool = "edit_file"
parameters = { path = "tracked.txt", old_string = "one", new_string = "changed" }"#,
    )
    .await;
    let (tool, params) = only_step(&plan);
    assert_eq!(tool, "edit_file");

    EditFileTool::new(workspace.path().to_str().expect("utf8"))
        .execute(&input(params))
        .await
        .expect("executes");
    let content = std::fs::read_to_string(workspace.path().join("tracked.txt")).expect("the file");
    assert!(content.contains("changed"), "{content}");
}

#[test]
fn the_harness_drives_every_action_the_dialect_declares() {
    // Read from the enum's own source: the set of `#[serde(rename_all = "snake_case")]` variants, which
    // is what a document's `type = "…"` may say. A tenth action fails this test until it is driven, which
    // is exactly the assert that the earlier attempt lacked.
    let source = include_str!("../../engine/src/templates/domain/template.rs");
    let enum_body = source
        .split("pub enum TemplateAction {")
        .nth(1)
        .expect("the enum")
        .split("\n}")
        .next()
        .expect("its body");

    let mut declared = Vec::new();
    for line in enum_body.lines() {
        let trimmed = line.trim_end();
        // A variant is `    Name {` (or `    Name,`) at one indent level, after its doc comment.
        let Some(name) = trimmed
            .strip_suffix(" {")
            .or_else(|| trimmed.strip_suffix(','))
        else {
            continue;
        };
        let name = name.trim();
        if !name.is_empty()
            && name.chars().next().is_some_and(|c| c.is_ascii_uppercase())
            && name.chars().all(|c| c.is_ascii_alphanumeric())
        {
            declared.push(to_snake_case(name));
        }
    }

    declared.sort();
    let mut expected: Vec<String> = declared
        .iter()
        .filter(|name| !REFUSED_BY_DESIGN.iter().any(|refused| refused == name))
        .cloned()
        .collect();
    expected.sort();

    let mut actual: Vec<String> = EXERCISED.iter().map(|name| name.to_string()).collect();
    actual.sort();

    assert_eq!(
        actual, expected,
        "the harness must drive every action the dialect declares, except the ones the conversion \
         refuses by design. Declared: {declared:?}; refused: {REFUSED_BY_DESIGN:?}"
    );
    assert_eq!(
        declared.len(),
        REFUSED_BY_DESIGN.len() + EXERCISED.len(),
        "the count is a sum with a reason, not a number that happens to match: {declared:?}"
    );
}

fn to_snake_case(name: &str) -> String {
    let mut out = String::new();
    for (index, character) in name.chars().enumerate() {
        if character.is_ascii_uppercase() {
            if index > 0 {
                out.push('_');
            }
            out.push(character.to_ascii_lowercase());
        } else {
            out.push(character);
        }
    }
    out
}
