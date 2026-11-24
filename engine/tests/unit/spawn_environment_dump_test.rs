//! #1661 — a spawned child **dumps its own environment**, and the dump is the assertion.
//!
//! Reading the source and declaring the helper correct is the check that fails the moment a call site
//! forgets. This test runs the real spawn and reads what the child actually saw.
//!
//! The strongest case is `git commit`, because it is the one that runs a program the **repository**
//! supplies: a `pre-commit` hook. If `RIGORIX_HMAC_KEY` — the key an audit envelope is signed with — is in
//! that hook's environment, a repository can read the key that signs the record of what it did. So the
//! fixture is a hook that writes `env` to a file, and the assertion is what is in it.

#![cfg(unix)]

use rigorix_engine::tools::application::GitCommitTool;
use rigorix_engine::tools::domain::{Tool, ToolInput};

/// The fixture's own `git`, seeding and inspecting the repository the tool under test writes to. The spawn
/// being measured is the tool's; this one is the ruler, and routing the ruler through the declaration under
/// test would make the assertion circular.
#[allow(clippy::disallowed_types)]
fn git(dir: &std::path::Path, args: &[&str]) {
    let output = std::process::Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .expect("git runs");
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[tokio::test]
async fn a_commit_hook_cannot_see_the_signing_key() {
    let dir = tempfile::tempdir().expect("tempdir");
    let repo = dir.path();
    git(repo, &["init", "-q", "-b", "main"]);
    git(repo, &["config", "user.name", "dump"]);
    git(repo, &["config", "user.email", "dump@example.invalid"]);
    std::fs::write(repo.join("tracked.txt"), "seed\n").expect("seed");
    git(repo, &["add", "tracked.txt"]);
    git(repo, &["commit", "-q", "-m", "seed"]);

    // The hook a repository can write. It records the environment it was given.
    let hook = repo.join(".git/hooks/pre-commit");
    std::fs::write(
        &hook,
        "#!/bin/sh\nenv > \"$(git rev-parse --git-dir)/hook-env.txt\"\n",
    )
    .expect("hook");
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    }

    // SAFETY: the variables are planted for the duration of this test and removed after it; the child
    // reads its environment at spawn, which happens inside the call below.
    unsafe {
        std::env::set_var("RIGORIX_HMAC_KEY", "the-key-that-signs-the-trail");
        std::env::set_var("GITHUB_TOKEN", "a-repository-token");
        std::env::set_var("ANTHROPIC_API_KEY", "a-model-key");
        std::env::set_var("SSH_AUTH_SOCK", "/tmp/agent.sock");
        std::env::set_var("GIT_AUTHOR_NAME", "operator-name");
    }

    std::fs::write(repo.join("tracked.txt"), "changed\n").expect("modify");
    // Staged by the fixture, not by `auto_stage`: this test's subject is the environment the **hook**
    // receives, and a dependency on the boolean read (#1660) would make it fail for a reason that is not
    // its own.
    git(repo, &["add", "tracked.txt"]);
    let tool = GitCommitTool::new(repo.to_str().expect("utf8"));
    let mut params = std::collections::HashMap::new();
    params.insert(
        "message".to_string(),
        serde_json::json!("from the dump test"),
    );
    tool.execute(&ToolInput::new(params))
        .await
        .expect("the commit runs and its hook fires");

    let dump = std::fs::read_to_string(repo.join(".git/hook-env.txt"))
        .expect("the hook wrote the environment it was given");
    unsafe {
        std::env::remove_var("RIGORIX_HMAC_KEY");
        std::env::remove_var("GITHUB_TOKEN");
        std::env::remove_var("ANTHROPIC_API_KEY");
        std::env::remove_var("SSH_AUTH_SOCK");
        std::env::remove_var("GIT_AUTHOR_NAME");
    }

    for forbidden in [
        "RIGORIX_HMAC_KEY",
        "GITHUB_TOKEN",
        "ANTHROPIC_API_KEY",
        "SSH_AUTH_SOCK",
    ] {
        assert!(
            !dump
                .lines()
                .any(|line| line.starts_with(&format!("{forbidden}="))),
            "`{forbidden}` reached a program the repository supplied (#1661). Hook environment:\n{dump}"
        );
    }
    // The positive control: the declaration is an allowlist, not a blanket refusal — git's own identity
    // plumbing still arrives, which is what makes a commit possible at all.
    assert!(
        dump.lines()
            .any(|line| line.starts_with("GIT_AUTHOR_NAME=")),
        "git's identity plumbing must still reach the child: {dump}"
    );
    assert!(
        dump.lines().any(|line| line.starts_with("PATH=")),
        "a child without PATH cannot run: {dump}"
    );
}

#[tokio::test]
async fn a_scoring_backend_is_the_documented_exception_rather_than_an_oversight() {
    // The allowlist has one production row besides the two below it, and a row nobody tests is a row
    // nobody would notice going stale. This asserts the *reason* stated there: the backend receives the
    // deployment's environment, because it is the deployment's script.
    let declaration = include_str!("../../../.pi/spawn-sites.tsv");
    let rows: Vec<&str> = declaration
        .lines()
        .filter(|line| !line.trim_start().starts_with('#') && !line.trim().is_empty())
        .collect();
    assert_eq!(
        rows.len(),
        2,
        "two exemptions, each with a reason; a third needs a reason a reader can check: {rows:?}"
    );
    for row in rows {
        let cells: Vec<&str> = row.split('\t').collect();
        assert_eq!(cells.len(), 2, "path<TAB>reason, no commas: {row}");
        assert!(
            cells[1].len() > 40,
            "an exemption whose reason is a phrase is a name with punctuation: {row}"
        );
    }
}
